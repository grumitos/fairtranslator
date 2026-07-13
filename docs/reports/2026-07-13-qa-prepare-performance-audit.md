# Auditoría de rendimiento de `qa_prepare`

**Fecha:** 2026-07-13
**Caso medido:** campaña `inakagurashi`, RPG Maker MZ, 33 archivos y 22.801 entradas
**Alcance:** construcción del plan y generación inicial de lotes; no se inspeccionó ni modificó texto del juego.

## Resultado

La lentitud no provenía principalmente de la falta de cache, locks o escritura
incremental. El cuello de botella estaba en `chunk_requests`: para decidir dónde
cortar un lote, reconstruía y serializaba el prompt completo al añadir casi cada
entrada. Con 22.801 entradas, el coste de presupuestación se repetía decenas de
miles de veces aunque el resultado final solo contuviera 133 lotes.

El planner ahora prueba primero el candidato máximo de 200 entradas. Solo cuando
ese candidato excede el presupuesto hace una búsqueda binaria del mayor corte
válido. Una prueba de regresión verifica que 1.000 entradas que caben en cinco
lotes requieren cinco evaluaciones completas, no una evaluación por entrada.

También se eliminó trabajo que el exportador no consumía: la estimación externa
ya no construye un bundle compacto completo para descartar su payload ni genera
la versión JSON con sangría cuando solo necesita la forma compacta.

## Mediciones

| Escenario | Perfil | Tiempo observado | Resultado |
|---|---:|---:|---|
| Implementación anterior, proceso de campaña | `dev` | más de 2.985 s de CPU | ningún lote visible durante la planificación |
| Planner corregido, solo carga y planificación | `release` | 0,93 s de pared; 0,81 s de planner | 133 lotes planeados |
| Preparación completa corregida | `release` | 2,759 s de pared | 133 lotes, 399 artefactos y manifest completo |

La compilación release no está incluida en el tiempo de ejecución: es un paso de
build, no parte de una campaña ejecutada con un binario ya distribuido.

## Decisión operativa

`qa_prepare` debe ejecutarse con el perfil `release` en campañas reales. El
perfil `dev` conserva comprobaciones y código sin optimizar adecuados para
depuración, pero en este caso medido todavía tardó alrededor de un minuto aun con
el algoritmo corregido. El runbook y la ayuda del comando ya muestran
`cargo run --release`.

No se implementaron cache persistente, fases reanudables ni locks como parte de
esta corrección: con una preparación completa menor a tres segundos, esas piezas
añadirían complejidad sin resolver el cuello de botella que produjo el incidente.
Siguen siendo mejoras independientes de robustez si una campaña futura demuestra
una necesidad operativa concreta.

## Verificación

- 15 pruebas del orquestador pasaron, incluidas tres nuevas pruebas del corte por presupuesto.
- 17 pruebas de exportación/aplicación de lotes pasaron.
- La corrida real release terminó con código 0.
- El manifest reportó 133 lotes y se materializaron 399 archivos de lote, tres por lote.
- Todos los workspaces de benchmark se crearon bajo `D:\FairTranslatorRuns`; la fuente del juego permaneció en modo lectura.
