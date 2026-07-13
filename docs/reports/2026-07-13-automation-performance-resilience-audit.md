# Auditoría extra: rendimiento y recuperación del trabajo automático

**Fecha:** 2026-07-13
**Campaña de referencia:** `inakagurashi -> es`
**Alcance:** preparación, traducción headless, checkpoints, respuestas de agente externo, aplicación, validación y exportación. No se enviaron solicitudes a proveedores ni se modificó el juego fuente.

## Resultado ejecutivo

La preparación inicial ya no es el cuello de botella: el proyecto real de 22.801 entradas se prepara en perfil release en 2,759 s. La auditoría adicional encontró que las búsquedas lineales, la aplicación y la serialización de checkpoints tampoco explican esperas grandes en esta escala.

El riesgo principal era de recuperación. Había dos formas de perder la continuidad aparente o real:

1. `qa_translate_provider` podía ser interrumpido durante el intercambio de directorios de `project.fairtl`. El checkpoint anterior seguía en un backup, pero al faltar momentáneamente la ruta canónica el siguiente proceso construía un proyecto nuevo.
2. Repetir `qa_prepare` sobre una campaña externa generaba nuevos IDs de proyecto y lote. Esto podía invalidar respuestas ya terminadas aunque sus archivos siguieran presentes.

Ambos casos quedan corregidos. El runner directo restaura el último checkpoint completo antes de decidir si reanuda y bloquea un segundo escritor. La preparación externa reutiliza el manifest y los lotes existentes únicamente si todos los metadatos y fingerprints coinciden; cuenta las respuestas completas y se niega a reconstruir IDs cuando ya hay respuestas y detecta deriva.

## Mediciones

Proyecto medido:

- 33 archivos traducibles.
- 22.801 entradas.
- Archivo mayor: 3.915 entradas.
- `project.fairtl`: 5.775.731 bytes.
- 133 lotes, 399 artefactos y 16.290.894 bytes en `ai-batches`.
- Árbol fuente: 4.454 archivos y 1.906.068.833 bytes.

Microbenchmark release, sin red y sin mostrar texto fuente:

| Operación | Tiempo |
|---|---:|
| Cargar proyecto | 63,3 ms |
| Construir 22.801 updates sintéticos | 7,8 ms |
| Aplicar todos los updates del flujo externo | 183,3 ms |
| Aplicar todos los updates en chunks de 200 del runner directo | 206,3 ms |
| Clonar 100 veces el archivo mayor para rollback | 367,4 ms |
| Guardar un checkpoint completo, media de 5 | 56,8 ms |
| Guardar un checkpoint completo, máximo | 65,5 ms |
| Validar cobertura | 25,1 ms |

Las búsquedas actuales realizan unas 22.666.241 comparaciones lineales para aplicar toda esta campaña, pero aun así la fase completa queda por debajo de 0,21 s en release. Indexarlas ahora reduciría complejidad, no el tiempo operativo dominante.

Los 133 checkpoints completos costarían aproximadamente 7,6 s acumulados usando la media observada. Mantener un checkpoint por chunk está justificado por la recuperación que aporta.

La prueba de reanudación de `qa_prepare` sobre los 133 lotes reales conservó el hash del manifest. El comando tardó 40,802 s, de los cuales 36,44 s correspondieron a recompilar el binario dev; la ejecución reutilizada fue de aproximadamente 4,36 s. Esto confirma que las campañas deben usar binarios release ya construidos y no pagar compilación durante una recuperación.

## Recuperación implementada

### Traducción directa por proveedor

- El backup de `project.fairtl` ahora tiene una ruta determinista asociada al proyecto.
- Si el proceso se detiene después de apartar el checkpoint anterior y antes de publicar el nuevo, el siguiente proceso restaura el backup antes de ejecutar `build_project`.
- También se recupera el formato de backup aleatorio de versiones anteriores cuando existe un único candidato dentro de un workspace QA reconocible. Si hay varios candidatos, la recuperación falla de forma segura por ambigüedad.
- El workspace mantiene un lock exclusivo durante toda la ejecución. En Windows el lock depende de un handle del sistema operativo, de modo que un proceso terminado lo libera automáticamente.
- `qa_translate.progress.log` se conserva y añade una nueva cabecera de sesión en vez de truncarse.
- El planner sigue seleccionando sólo entradas pendientes. Si el corte ocurrió antes de publicar el checkpoint, se repite ese chunk; si el checkpoint ya era visible, sus entradas no vuelven a planificarse.

La secuencia durable queda así:

```text
checkpoint N canónico
  -> aplicar chunk N+1 en memoria
  -> escribir proyecto nuevo en temporal
  -> mover checkpoint N a backup determinista
  -> publicar proyecto nuevo
  -> eliminar backup
```

Un corte en cualquier punto conserva el checkpoint N o deja publicado N+1.

### Flujo de agente externo

Al repetir `qa_prepare` con el mismo workspace y opciones:

- se verifica ruta resuelta, engine, idiomas, chunk size cuando está disponible, número de archivos y entradas, fingerprint del inventario y directorio de lotes;
- se carga y verifica `project.fairtl`;
- se valida cada índice, ID, fingerprint de entradas, prompt, archivo de entradas y template;
- se comprueba que cada template pertenezca al proyecto y lote esperados;
- se cuentan sólo los `.response.json` parseables, con IDs correctos y cobertura exacta;
- si todo coincide, no se reescribe el manifest ni se regeneran UUID;
- si algo cambió y existe cualquier respuesta, se aborta en vez de invalidar trabajo terminado.

Cada agente externo debe publicar una respuesta mediante archivo temporal y rename. Un JSON parcial queda clasificado como respuesta inválida y se vuelve a procesar; no cuenta como chunk completo.

## Cuellos de botella restantes

### P1: exportación completa del árbol del juego

La fuente real ocupa 1,906 GB. `export_project` copia el árbol completo antes de reescribir los archivos traducidos. Una interrupción durante esa copia obliga actualmente a empezar otra exportación; `qa_apply` además crea directorios de exportación con timestamp, por lo que pueden quedar árboles parciales grandes.

Mejora recomendada:

1. Exportar a un staging estable por fingerprint de fuente y proyecto.
2. Mantener un journal de archivos copiados con tamaño y metadatos suficientes para continuar.
3. Escribir cada archivo traducido a temporal y renombrarlo al finalizar.
4. Publicar el directorio final sólo después del readback.
5. Limpiar staging obsoleto únicamente cuando no coincida el fingerprint.

Para juegos MV/MZ con `www` anidado, conviene añadir un modo explícito `technical`, `playable` o `both`; generar ambos puede duplicar gigabytes sin aportar valor cuando sólo se necesita uno.

### P1: compilación dentro del flujo operativo

`cargo run` puede dominar una recuperación aunque el runner sea rápido. El runbook usa ahora `--release`, pero para campañas repetidas es mejor construir una vez y ejecutar directamente los binarios de `target/release` mientras el código no cambie.

### P2: reparses y lookups

- `qa_apply` valida la fuente y el exportador vuelve a parsearla antes de copiar; después el readback parsea la salida. El readback es necesario, pero las dos lecturas previas pueden compartir un snapshot verificado.
- `apply_ai_batch_updates` y `apply_provider_responses` usan búsquedas lineales. Un índice `(file_path, entry_id)` sería recomendable para campañas mucho mayores, pero sólo ahorraría alrededor de dos décimas de segundo en esta campaña.
- Los snapshots de rollback clonan el archivo completo por chunk. El coste medido es bajo; un journal por entradas sólo merece la pena si aparecen archivos muy superiores a 3.915 entradas.

### No recomendado por defecto: paralelizar proveedores

Ejecutar chunks concurrentes puede reducir tiempo de red, pero cambia límites de uso, coste, orden de contexto, presión de rate limit y semántica de fallback. Debe ser una opción explícita con concurrencia baja y límites por proveedor, no una optimización automática.

## Pruebas de regresión añadidas

- Recuperación cuando el checkpoint canónico fue movido a backup durante el swap.
- Recuperación de un único backup legacy dentro de un workspace QA.
- Tras recuperar, el planner incluye sólo entradas posteriores al último chunk confirmado.
- Un segundo escritor no puede adquirir el mismo workspace; tras liberar el lock, otro sí puede.
- Una preparación externa válida reutiliza IDs y cuenta respuestas completas.
- Una preparación con deriva se niega a invalidar respuestas existentes.
- Prueba real sobre 133 lotes: manifest sin cambios tras reanudar.

## Criterio operativo

Para obtener recuperación real hay que conservar exactamente el mismo workspace:

- Runner directo: repetir el mismo comando y `--workspace`.
- Agente externo: repetir `qa_prepare` con el mismo workspace y opciones, y continuar sólo respuestas ausentes o inválidas.

Cambiar fuente, idiomas o tamaño de chunk es una campaña diferente. Cuando ya existen respuestas, debe usarse un workspace nuevo en lugar de reconstruir silenciosamente el anterior.

## Verificación ejecutada

- `cargo fmt --all -- --check`: correcto.
- `cargo test`: 271 pruebas pasaron, 0 fallaron y 1 generador de bindings quedó ignorado por diseño.
- `cargo check --release --bins`: correcto para todos los binarios release.
- `cargo clippy --all-targets --all-features`: terminó con código 0. Conserva 18 avisos preexistentes en librería, `qa_apply` y una función antigua de `qa_translate_provider`; los cambios nuevos de preparación, persistencia y lock no añadieron avisos.
- Reanudación real de `qa_prepare` sobre 133 lotes: código 0 y hash del manifest sin cambios.

No se ejecutaron proveedores, `qa_apply` real ni exportación completa: habría enviado trabajo externo o escrito aproximadamente 1,9 GB sin ser necesario para demostrar estas mejoras.

## Seguimiento: credenciales, tamaño adaptativo y esquema OpenCode

Una ejecución posterior del mismo proyecto mostró `1.442` chunks directos para
`22.801` entradas. La distribución confirmó que no era un límite de contexto:
`1.410` chunks contenían exactamente 16 entradas, el máximo fijo de OpenCode.
El percentil 95 tenía sólo 357 caracteres originales por chunk.

La simulación por archivo sobre el inventario real dio esta línea base, sin
mostrar texto del juego:

| Máximo de entradas | Chunks aproximados | P95 caracteres | P95 salida estimada |
|---:|---:|---:|---:|
| 16 | 1.442 | 357 | 271 tokens |
| 64 | 377 | 1.283 | 1.026 tokens |
| 128 | 200 | 2.439 | 2.000 tokens |
| 200 | 133 | 3.631 | 3.053 tokens |

Se sustituyó el máximo fijo de 16 por un máximo de 128 con dos límites
adaptativos: presupuesto total estimado de 18.000 tokens y presupuesto de salida
estimada de 3.200 tokens frente a un máximo solicitado de 4.096. Las entradas
cortas se agrupan; las largas se dividen antes. El fingerprint del planner se
versionó para descartar caches con la política anterior sin perder checkpoints.

El hilo operativo también mostró fallos de esquema con `minimax-m3`, incluso al
reintentar entradas individuales. La causa era de negociación: FairTranslator
aceptaba el primer contenido no vacío de una familia API y sólo después intentaba
parsearlo. Ahora la validación inicial exige una traducción estructurada real,
prueba familias hasta obtener un resultado completo y recuerda la familia válida
para el resto de la instancia. Si todas fallan, el runner conserva su aislamiento
y recuperación existentes.

El runner headless carga por defecto la misma credencial de OpenCode que la UI
guarda en el almacén seguro del sistema operativo. `--api-key-file` se conserva
como compatibilidad explícita, no como flujo de producto.

Para agentes externos, los 133 lotes preparados con tamaño 200 ya respetaban el
presupuesto de prompt. La paralelización recomendada es un pool acotado de
workers que reclame lotes independientes y publique cada respuesta atómicamente;
no hace falta crear un agente por chunk ni acumular toda la campaña en una sola
sesión susceptible de compactación.
