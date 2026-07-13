# Informe de revisión: construcción inicial de la campaña `inakagurashi → español`

**Fecha:** 2026-07-13
**Estado de la campaña:** preparación pausada antes del envío a OpenCode
**Fuente del informe:** flujo operativo de FairTranslator, código del planner y artefactos de ejecución
**Alcance de inspección:** orquestación y rendimiento. No se inspeccionó el contenido textual extraído del juego.

## Resumen ejecutivo

La ruta elegida es la correcta para mantener el juego original intacto: preparar el proyecto fuera de la fuente, generar lotes, entregar cada lote al agente externo, revisar respuestas y aplicar/exportar solo al final.

La construcción inicial presenta un problema operativo importante: `qa_prepare` ya creó la copia externa de trabajo, pero después de más de cincuenta minutos de CPU todavía no ha escrito ningún archivo en `ai-batches`. El proceso sigue respondiendo y consumiendo CPU, por lo que no parece un bloqueo de OpenCode. La causa más probable es que la planificación completa se construye en memoria y todos los artefactos se escriben únicamente después de que termina.

La optimización prioritaria es convertir la preparación en una operación reanudable y observable: lock por workspace, cache persistente del planner, progreso por archivo/lote y escritura incremental de lotes con manifest atómico.

## Estado observado

- **Juego fuente:** `D:\Biblioteca\Descargas\[Ryuugames] RY-RJ01650793\RJ01650793\inakagurashi`
- **Workspace externo:** `D:\FairTranslatorRuns\inakagurashi-es-20260713`
- **Copia de trabajo creada:** `01-mz-inakagurashi\project.fairtl`
- **Archivos copiados al proyecto externo:** 34
- **Entradas conocidas por el plan:** 22.801
- **Inventario:** terminado mediante `qa_inventory`; la salida se conservó en `inventory.command.log`.
- **Preparación:** una ejecución activa de `qa_prepare`.
- **Lotes materializados:** 0 archivos en `01-mz-inakagurashi\ai-batches` al momento de este informe.
- **Respuestas OpenCode:** ninguna enviada ni recibida.
- **Aplicación/exportación/validación:** todavía no iniciadas.
- **Fuente original:** no modificada.

Durante la ejecución se detectaron dos procesos antiguos del mismo `qa_prepare` apuntando al mismo workspace. Fueron detenidos para recuperar la regla de un único propietario de escritura. Se dejó una sola ejecución activa.

## Flujo que debe conservarse

El runbook distingue dos rutas de consola:

1. `qa_translate_provider --provider opencode` para una traducción headless directa mediante OpenCode Go.
2. Flujo de agente externo: `qa_prepare` genera `prompt.txt`, `entries.json` y `response.template.json`; el agente escribe el archivo hermano `response.json`; después `qa_apply` valida y aplica las respuestas.

Para esta campaña conviene conservar la segunda ruta porque separa claramente preparación, envío, revisión y aplicación. Además, permite mantener el juego original inmutable hasta que todos los lotes hayan sido procesados y revisados.

## Hallazgos técnicos

### 1. La preparación importa el juego dos veces

`qa_prepare` llama a `parse_game` dos veces consecutivas para comparar engine y fingerprint antes de construir el proyecto. Esto protege contra imports inestables, pero duplica el coste completo de lectura y parseo.

**Referencia:** `D:\Dev\fairtranslator\src-tauri\src\bin\qa_prepare.rs`, alrededor de las líneas 276-281.

**Mejora propuesta:** guardar el resultado de la primera importación junto con su fingerprint y hacer la segunda lectura solo mediante una opción explícita como `--verify-stability`. Para campañas normales, verificar metadatos y fingerprint antes de aplicar puede ser suficiente. La comprobación profunda debe seguir disponible para diagnósticos.

### 2. El planner se ejecuta sin cache persistente

La función pública `build_external_ai_plan` usa la variante sin cache. Aunque existe `PlannerCacheData` y una API con progreso, `qa_prepare` no conserva esa cache entre ejecuciones. Si el proceso se interrumpe después de decenas de minutos, la siguiente ejecución vuelve a planificar desde cero.

**Referencia:** `D:\Dev\fairtranslator\src-tauri\src\services\translation_orchestrator.rs`, alrededor de las líneas 237-299.

**Mejora propuesta:** guardar `planner-cache.json` o una representación binaria versionada dentro del workspace. La cache debe incluir versión del parser, fingerprint de inventario, fingerprint de guía y tamaño de lote. Si alguno cambia, se invalida de forma segura.

### 3. Se construye todo el conjunto de lotes antes de escribir el primero

`export_ai_batches` obtiene todos los chunks, transforma todos los exports y los recoge en memoria antes de devolverlos. Solo después `qa_prepare` entra en el bucle que escribe `prompt.txt`, `entries.json` y `response.template.json`.

**Referencias:**

- `D:\Dev\fairtranslator\src-tauri\src\services\ai_batch.rs`, alrededor de las líneas 261-276.
- `D:\Dev\fairtranslator\src-tauri\src\bin\qa_prepare.rs`, alrededor de las líneas 311-355.

Este diseño explica por qué `ai-batches` permanece vacío mientras el proceso consume CPU: no hay escritura incremental ni evidencia de avance.

**Mejora propuesta:** exponer un iterador/callback de chunks. Cada chunk debe escribirse en archivos temporales y renombrarse atómicamente al terminar. El manifest solo debe publicarse cuando todos los lotes hayan pasado el fingerprint y estén completos.

### 4. El progreso del planner se descarta

La variante usada por `build_external_ai_plan` conecta el planner con un callback `noop`. Por eso la consola no informa el archivo actual, entradas pendientes, lotes planeados ni tiempo transcurrido.

**Referencia:** `D:\Dev\fairtranslator\src-tauri\src\services\translation_orchestrator.rs`, alrededor de las líneas 237-251.

**Mejora propuesta:** conectar `PlanningProgressSnapshot` con stderr o con un log estructurado. Como mínimo, registrar:

- fase actual;
- archivos completados y totales;
- entradas pendientes;
- lotes creados;
- archivo actual;
- duración y velocidad aproximada.

El progreso debe mostrar metadatos, no texto del juego.

### 5. No existe un lock de workspace

La misma campaña aceptó tres procesos idénticos sobre el mismo workspace. Esto expone el proyecto a carreras de escritura, manifests inconsistentes y respuestas mezcladas.

**Mejora propuesta:** crear `workspace.lock` con PID, host, inicio, comando y versión del runner. Rechazar una segunda ejecución mientras el lock esté activo. Añadir una recuperación explícita para locks huérfanos, con comprobación del PID y confirmación de que no existe un proceso vivo.

### 6. Se utiliza el perfil de desarrollo para una carga grande

El runbook muestra `cargo run`, que ejecuta el binario en el perfil `dev`. Para un proyecto con decenas de miles de entradas, el coste del planner y la serialización puede ser considerablemente mayor que en `--release`.

**Mejora propuesta:** documentar y medir una ruta de campaña con binarios release:

```powershell
cargo run --release --manifest-path D:\Dev\fairtranslator\src-tauri\Cargo.toml --bin qa_prepare -- ...
```

No debe cambiarse automáticamente sin benchmark. Primero hay que comparar tiempo de importación, planificación, serialización y memoria.

### 7. La operación no tiene fases reanudables

Actualmente importación, persistencia, planificación, generación de prompts y manifest se ejecutan como una sola operación. Un fallo al final obliga a repetir fases costosas.

**Mejora propuesta:** separar internamente o mediante flags:

```text
prepare import       -> project.fairtl + inventory fingerprint
prepare plan         -> planner cache + plan manifest
prepare export       -> batch artifacts + response templates
prepare verify       -> fingerprints y cobertura
```

Cada fase debe ser idempotente y poder continuar desde artefactos ya válidos.

## Diseño recomendado para la próxima versión

### Preparación

1. Resolver la carpeta y ejecutar un inventario silencioso con salida de metadatos.
2. Adquirir el lock del workspace.
3. Importar una vez y guardar `project.fairtl` junto con el fingerprint.
4. Cargar cache del planner si coincide con parser, guía, idioma, fuente y chunk size.
5. Planificar con progreso estructurado.
6. Escribir cada lote terminado en `ai-batches` mediante archivos temporales y rename atómico.
7. Cerrar y publicar el manifest únicamente cuando el conjunto esté completo.

### Envío externo

1. Seleccionar el modelo OpenCode Go dinámicamente mediante `/models` sin imprimir credenciales.
2. Probar el modelo con las muestras definidas en el plan.
3. Enviar cada `prompt.txt` mediante comandos, sin UI.
4. Validar que cada `response.json` sea hermano del lote, tenga proyecto/lote correctos y cubra cada entrada exactamente una vez.
5. Mantener los lotes disjuntos y un único propietario por respuesta.

### Aplicación y cierre

1. No ejecutar `qa_apply` hasta que todos los lotes estén presentes y revisados.
2. Aplicar en el workspace, nunca sobre la fuente original.
3. Ejecutar `qa_validate_translation --strict-unchanged --json`.
4. Exportar `export-es` y `playable-es` cuando corresponda.
5. Ejecutar smoke test o validación de parseo documentada.
6. Liberar el lock y conservar logs, manifest, fingerprints, fallos y selección de modelo.

## Prioridad de implementación

### P0, antes de otra campaña grande

- Lock exclusivo por workspace.
- Progreso visible por archivo y lote.
- Escritura incremental de lotes.
- Cache persistente o checkpoint del planner.
- Manifest atómico y verificación de cobertura.

### P1, mejora de rendimiento

- Benchmark `dev` frente a `release`.
- Importación única por defecto y verificación profunda opcional.
- Evitar acumular todos los `AiBatchExport` en memoria.
- Reutilizar `PlannerCacheData` entre reanudaciones.

### P2, ergonomía operativa

- `--dry-run --summary` que informe archivos, entradas, lotes estimados y fingerprints sin mostrar texto.
- Fases explícitas `import`, `plan`, `export` y `verify`.
- Comando para listar estado de campaña sin leer contenido textual.
- Métricas de tiempo y velocidad por fase.

## Criterios de aceptación de la optimización

La mejora debe considerarse lista cuando:

- una segunda ejecución concurrente sea rechazada limpiamente;
- una interrupción permita reanudar sin rehacer importación y planificación válidas;
- aparezca progreso periódico sin exponer texto del juego;
- los primeros lotes se escriban antes de terminar toda la campaña;
- el manifest no pueda publicarse incompleto;
- la fuente original permanezca sin cambios;
- un benchmark documente tiempo, CPU, memoria y número de lotes antes y después;
- `qa_apply` y `qa_validate_translation` sigan rechazando fingerprints obsoletos o respuestas incompletas.

## Conclusión

El flujo de consola y la separación entre fuente, workspace y agente externo son sólidos. El cuello de botella actual no está en el envío a OpenCode, sino en la construcción monolítica del plan: doble importación, planificación sin cache visible, acumulación de todos los chunks en memoria y escritura diferida.

La primera optimización debe mejorar la observabilidad y la reanudación sin cambiar el contrato de seguridad. Después conviene medir `--release`, eliminar trabajo duplicado y convertir el exportador en un productor incremental de lotes. Estas mejoras permitirán revisar el proyecto con evidencia y evitar que una campaña grande vuelva a parecer detenida aunque siga consumiendo CPU.
