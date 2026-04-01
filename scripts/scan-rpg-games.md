# `scan-rpg-games.mjs`

Script de inventario para revisar carpetas con juegos y detectar, por firma de archivos, qué motor usa cada uno.

Su objetivo es ayudar en la revisión manual previa al uso de FairTranslator, sin modificar el proyecto ni depender del backend de la aplicación.

## Qué hace

- Escanea una carpeta raíz de forma recursiva.
- Detecta carpetas que parecen ser raíces de juego.
- Identifica si el motor está soportado por FairTranslator o si solo puede clasificarse de forma heurística.
- Muestra una salida legible en consola o una salida estructurada en JSON.

## Qué no hace

- No abre ni modifica proyectos `.fairtl`.
- No traduce ni exporta archivos.
- No garantiza que toda carpeta detectada sea un juego funcional.
- No garantiza detección exhaustiva de todos los motores posibles.

## Requisitos

- Node.js disponible en el sistema.
- Ejecutar el script desde el repositorio o indicar la ruta absoluta al archivo.

## Uso

Desde la raíz del repositorio:

```powershell
node scripts/scan-rpg-games.mjs
```

Escanear una carpeta concreta:

```powershell
node scripts/scan-rpg-games.mjs D:\Juegos
```

Salida JSON:

```powershell
node scripts/scan-rpg-games.mjs --json D:\Juegos
```

Limitar profundidad de escaneo:

```powershell
node scripts/scan-rpg-games.mjs --max-depth 8 D:\Juegos
```

Ocultar firmas detectadas en la salida de texto:

```powershell
node scripts/scan-rpg-games.mjs --no-markers D:\Juegos
```

Ayuda integrada:

```powershell
node scripts/scan-rpg-games.mjs --help
```

## Argumentos y opciones

- `ruta`
  - Carpeta raíz a escanear.
  - Si no se pasa, usa la carpeta actual.

- `--json`
  - Imprime un objeto JSON con todos los resultados.

- `--max-depth N`
  - Define la profundidad máxima de búsqueda recursiva.
  - El valor por defecto es `6`.

- `--no-markers`
  - Oculta las firmas detectadas en la salida de texto.

- `--help` o `-h`
  - Muestra la ayuda resumida del script.

## Motores soportados por FairTranslator

Estos motores se consideran soportados porque son los que el proyecto ya reconoce actualmente:

- `RPG Maker MZ`
- `RPG Maker MV`
- `RPG Maker VX Ace`
- `RPG Maker VX`
- `RPG Maker XP`

## Motores no soportados detectados por heurística

El script también intenta identificar motores comunes que hoy no forman parte del soporte del proyecto:

- `RPG Maker 2000/2003`
- `Wolf RPG Editor`
- `Kirikiri / KAG`
- `TyranoScript / TyranoBuilder`
- `Ren'Py`
- `Unity`
- `Godot`
- `GameMaker`

En estos casos la detección es orientativa. La carpeta puede ser un juego real, una build incompleta o incluso un falso positivo si comparte la misma estructura base.

## Firmas usadas para detectar motores

### Soportados

- `RPG Maker MZ`
  - `js/rmmz_core.js`
  - `data/`

- `RPG Maker MV`
  - `js/rpg_core.js`
  - `data/`

- `RPG Maker VX Ace`
  - `Data/System.rvdata2`
  - o cualquier archivo `Data/*.rvdata2`

- `RPG Maker VX`
  - `Data/System.rvdata`
  - o cualquier archivo `Data/*.rvdata`

- `RPG Maker XP`
  - `Data/System.rxdata`
  - o cualquier archivo `Data/*.rxdata`

### No soportados o heurísticos

- `RPG Maker 2000/2003`
  - `RPG_RT.ldb`
  - `RPG_RT.lmt`
  - `RPG_RT.ini`
  - `RPG_RT.exe`
  - `Map*.lmu`

- `Wolf RPG Editor`
  - `Data.wolf`
  - `Data/BasicData/Game.dat`
  - `Data/BasicData/BasicData.dat`

- `Kirikiri / KAG`
  - `*.xp3`
  - `config.tjs`
  - `startup.tjs`
  - `krkr*.exe`

- `TyranoScript / TyranoBuilder`
  - `tyrano/`
  - `data/scenario/`
  - `tyrano/plugins/kag/kag.js`
  - `data/scenario/*.ks`

- `Ren'Py`
  - `renpy/`
  - `game/`
  - `game/*.rpa`
  - `game/*.rpy`

- `Unity`
  - carpeta `*_Data/`
  - `UnityPlayer.dll`

- `Godot`
  - `*.pck`
  - `*.exe`

- `GameMaker`
  - `data.win`
  - `game.unx`
  - `game.ios`
  - `options.ini`

## Formato de salida en texto

La salida de texto separa los resultados en dos grupos:

- `Soportados`
- `No soportados o heuristica`

Cada resultado incluye:

- motor detectado
- ruta relativa dentro de la carpeta raíz
- nivel de confianza en motores heurísticos: `high`, `medium` o `low`
- firmas detectadas, salvo que se use `--no-markers`

Ejemplo:

```text
Raiz: D:\Juegos
Soportados por FairTranslator: RPG Maker MZ, RPG Maker MV, RPG Maker VX Ace, RPG Maker VX, RPG Maker XP
Juegos detectados: 3

Soportados:
- RPG Maker MZ :: JuegoA
  firmas: js/rmmz_core.js, data/

No soportados o heuristica:
- Wolf RPG Editor :: JuegoB [medium]
  firmas: Data.wolf, Data/BasicData/Game.dat
```

## Formato de salida JSON

Con `--json`, el script emite un objeto con esta forma:

```json
{
  "scanRoot": "D:\\Juegos",
  "supportedByProject": [
    "RPG Maker MZ",
    "RPG Maker MV",
    "RPG Maker VX Ace",
    "RPG Maker VX",
    "RPG Maker XP"
  ],
  "results": [
    {
      "path": "D:\\Juegos\\JuegoA",
      "relativePath": "JuegoA",
      "engine": "RPG Maker MZ",
      "supported": true,
      "confidence": "high",
      "markers": [
        "js/rmmz_core.js",
        "data/"
      ]
    }
  ]
}
```

## Comportamiento de escaneo

- El script usa la carpeta actual si no recibe una ruta.
- El escaneo es recursivo y por defecto llega hasta profundidad `6`.
- Cuando detecta una raíz de juego, la registra y no sigue descendiendo dentro de esa carpeta.
- Ignora carpetas típicas de desarrollo o artefactos:
  - `.git`
  - `.idea`
  - `.vscode`
  - `.fairtl`
  - `build`
  - `dist`
  - `node_modules`
  - `target`
  - `__pycache__`

## Limitaciones conocidas

- Algunas detecciones no soportadas son necesariamente heurísticas.
- `Godot` y `GameMaker` usan firmas más genéricas y pueden producir más falsos positivos que otros motores.
- `RPG Maker 2000` y `RPG Maker 2003` se agrupan en una sola categoría.
- El script no inspecciona contenido interno de archivos, solo nombres y estructura de carpetas.
- Si un juego está empaquetado de una forma no estándar, puede no ser detectado.

## Cuándo usar este script

Úsalo cuando necesites:

- inventariar una carpeta grande de juegos
- separar rápidamente los juegos compatibles con FairTranslator
- detectar motores candidatos no soportados antes de una revisión manual
- generar una base JSON para clasificar juegos antes de importarlos al flujo del proyecto
