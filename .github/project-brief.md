# project-brief: FairTranslator

## Translator++

Translator++ es el programa de referencia actual para traducir juegos creados con motores como RPG Maker y Wolf RPG Editor. Fue creado por Dreamsavior originalmente para traducir juegos *doujin* japoneses, combinando traducción automática con edición manual.

Su flujo de trabajo central es un **editor en grid**: cada fila representa una línea de texto del juego, y el usuario puede tener múltiples versiones de traducción por fila, usando la más reciente como resultado final al exportar.

Soporta motores como RPG Maker XP, VX, VX Ace, MV, MZ, Wolf RPG Editor, KAG/Kirikiri y LiveMaker. Integra servicios como Google Translate, DeepL, Sugoi Translator y ChatGPT (vía add-ons), y tiene funciones como búsqueda/reemplazo con regex, tracking visual de progreso y word wrap automático.

---

## Problem

Translator++ ha evolucionado hacia un modelo de monetización confuso y agresivo:

- La versión base es gratuita pero las funciones más útiles (como integración con IA) están bloqueadas detrás de **add-ons de pago**.
- El pago no es una licencia única: se accede mediante **Patreon**, que otorga créditos mensuales.
- Los créditos acumulados se canjean por add-ons. Un add-on puede costar 4,000 créditos, pero la suscripción mensual entrega solo 1,000, obligando a pagar varios meses antes de poder desbloquear una sola función.
- El resultado es un costo real mayor al de muchos programas profesionales con licencia única, pero disfrazado de suscripción mensual accesible.

Adicionalmente, la interfaz del programa es visualmente anticuada y con una experiencia de usuario tosca.

---

## Solution

Crear una herramienta nueva, moderna y sin fricciones de monetización, enfocada inicialmente en los motores más accesibles técnicamente.

Principles:
- El usuario conecta **sus propias API keys** (OpenAI, DeepL, Google). Sin créditos intermediarios. El costo real de traducir un juego completo con modelos como GPT-4o-mini ronda los $0.50–$2 USD.
- Interfaz moderna y limpia desde el primer día.
- Open source o licencia única simple — sin suscripciones.
- Comenzar por RPG Maker MV/MZ, donde los archivos son JSON estándar sin encriptación, con estructuras predecibles. El parsing es trivial.

---

## Engine Complexity

| Motor | Formato | Dificultad |
|---|---|---|
| RPG Maker MV / MZ | JSON puro en `/data/*.json` | baja |
| RPG Maker VX Ace | Ruby Marshal binario `.rvdata2` | media |
| RPG Maker XP / VX | Ruby Marshal `.rxdata` / `.rvdata` | media |
| Wolf RPG Editor | Binario propietario | alta |
| KAG / Kirikiri | Scripts `.ks` + `.tjs` | media |

---

## MVP Scope

### core
- Parser para RPG Maker MV/MZ (lectura y escritura de JSON)
- UI de tabla/grid editable
- Sistema de proyectos (guardar estado de traducción)
- Exportar / parchear archivos originales con las traducciones

### ai integration
- Conexión directa a OpenAI, DeepL, Google Translate con API key del usuario
- Cola de traducción en batch
- Glosario de términos para consistencia de nombres propios

### quality of life
- Find & Replace con regex
- Tracking visual de progreso por archivo
- Word wrap automático configurable

---

## Stack Recommendations (open)

### Option A — Electron + React
- Acceso completo al filesystem nativo
- UI moderna (ej. shadcn/ui, Tailwind)
- Fácil distribución multiplataforma
- contra: binario pesado, rendimiento inferior en archivos grandes

### Option B — Tauri + React
- Binario significativamente más pequeño que Electron
- Backend en Rust: excelente para parsing de archivos pesados
- Frontend React moderno
- contra: mayor complejidad de setup inicial, ecosistema más joven

### Option C — Python + PyQt / PySide
- Prototipado más rápido
- Librerías de parsing Marshal ya existen en Python
- contra: UI menos pulida por defecto, distribución más engorrosa

---

## Timeline Estimate (MV/MZ MVP)

| Fase | Estimado |
|---|---|
| Parser MV/MZ + grid UI + export | 1–2 semanas |
| Integración DeepL / OpenAI | 2–3 días |
| Glosario, find/replace, tracking | 1 semana |
| Parser VX Ace | 1 semana adicional |
| Packaging y distribución | 2–3 días |

total MV/MZ functional MVP: ~2-3 weeks sustained work.
