# Phase 1: Operation-First Document Model - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-11-24
**Phase:** 1-Operation-First Document Model
**Areas discussed:** Operación efectiva al migrar, Formato v1/v2, Capas plantilla/guía, Campos legacy y guardado, Tool en la capa, Presets y profiles, Settings en fase 1, Documento nuevo y capas sin operación, Asignación de operación a capa, Orden intra-capa

---

## Operación efectiva al migrar

**Caso IRL:** una capa v1 `Cut` contiene un shape con `production.operation = Score` o `style = Engrave`. G-code usa `Layer.kind`; layout usa `Shape::operation()`.

| Option | Description | Selected |
|--------|-------------|----------|
| Gana el shape; partir capa | Agrupar por operación efectiva; una capa por grupo | ✓ |
| Gana Layer.kind | Normalizar todos los shapes a la capa | |
| Híbrido: split solo si difiere | Mantener capa y extraer solo los que difieren | |
| Mantener override legacy | Capa con default y override por shape | |

**User's choice:** Gana el shape; partir capa.
**Notes:** Después del caso IRL, se confirmó que `Shape.tool` también debe subir a capas. La operación efectiva sirve para reconstruir capas; después no es autoridad.

---

## Formato v1 o v2

**Caso IRL:** un job nuevo con N:M se abre en un StencilCraft viejo. Si sigue siendo v1, el build viejo puede cargar una escena vacía o guardar con pérdida.

| Option | Description | Selected |
|--------|-------------|----------|
| Subir a v2 | Builds viejos rechazan con error de versión; v1 se migra al cargar | ✓ |
| Mantener v1 aditivo | Campos opcionales en v1; riesgo de guardado con pérdida | |
| v2 + proyección v1 | Escribir v2 y exportar copia v1 si no hay N:M | |
| Solo v2, sin migrar al cargar | Migración explícita | |

**User's choice:** Subir a v2.
**Notes:** v1 se migra en memoria; el archivo no se reescribe al abrir.

---

## Capas plantilla/guía

**Caso IRL:** una plantilla de registro se ve en pantalla pero la máquina no la ejecuta.

| Option | Description | Selected |
|--------|-------------|----------|
| Operation::Guide | Toda capa tiene operación; plantilla usa Guide | ✓ (luego revisado) |
| Operación opcional | Solo capas no-plantilla exigen operación | |
| Plantilla fuera de Layer | Entidad separada | |
| Operación libre + flag | Cualquier operación y flag template | |

**User's choice:** Inicialmente `Operation::Guide`; después se corrigió a operación opcional.
**Notes:** La operación es un marcador opcional; template/guide puede ser `None` o `Guide`. La decisión final está en "Documento nuevo y capas sin operación".

---

## Campos legacy y guardado

**Caso IRL:** un taller abre un job v1, lo revisa y lo guarda. No quiere reescritura silenciosa ni campos legacy que confundan.

| Option | Description | Selected |
|--------|-------------|----------|
| Solo lectura/legacy | Se deserializan para migrar; no son autoridad ni se escriben en v2 | ✓ |
| Eliminarlos del modelo | DTO separado para v1 | |
| Mantenerlos escribibles | Fallback/override | |
| Mantener solo kind/style | Conservar compatibilidad visual | |

**User's choice:** Solo lectura/legacy.

| Option | Description | Selected |
|--------|-------------|----------|
| Migrar al cargar, guardar v2 | Migración en memoria; v2 solo al guardar | ✓ |
| Migración explícita | El documento sigue v1 hasta comando | |
| Reescribir al abrir | Migrar y guardar con backup | |
| Guardar v1 salvo elección | v1 por defecto; "Guardar como v2" | |

**User's choice:** Migrar al cargar, guardar v2.

---

## Tool en la capa

**Caso IRL:** laser cut/engrave usan la misma herramienta; score usa rodillo; punch usa punzón; mill profile/pocket usan la misma fresa con parámetros distintos. `Operation::Score` y `ToolOp::Score` duplican.

| Option | Description | Selected |
|--------|-------------|----------|
| Operación única con parámetros | Una operación por capa, con settings | Revisado |
| kind + settings | `OperationKind` + `OperationSettings` | |
| Process + Strategy | Dos ejes internos, operaciones cocinadas | ✓ (síntesis) |
| Tool sigue en el shape | `Shape.tool` manda | |

**User's choice:** Dos ejes internos (proceso + estrategia), pero la UI y el documento usan operaciones precocinadas y paramétricas. `ToolOp` desaparece como autoridad.
**Notes:** "Fill shape" y "pocket" son estrategias, no herramientas. La representación Rust exacta queda a planificación.

---

## Presets y profiles

**Caso IRL:** 20 capas usan "Corte 3 mm MDF"; una capa necesita ajuste local. No deben cambiar las otras 19.

| Option | Description | Selected |
|--------|-------------|----------|
| Snapshot + preset_id opcional | Documento portable; preset solo procedencia | ✓ |
| Referencia al profile | Profile ausente rompe el documento | |
| Snapshot enlazado | Snapshot + vínculo opcional | |
| Snapshot + profile versionado | Referencia inmutable | |

**User's choice:** Snapshot + preset_id opcional, con opción de extraer perfiles embebidos como estilos de párrafo.

| Option | Description | Selected |
|--------|-------------|----------|
| Assign + Link opcional | Assign copia; Link sincroniza y bloquea settings | ✓ |
| Solo Assign | Sin vínculo vivo | |
| Solo Link | Sin overrides locales | |
| Link con override local | Más flexible que LightBurn | |

**User's choice:** Assign + Link opcional, modelo LightBurn.
**Notes:** "Create new from layer" = extraer preset desde la capa activa. El snapshot embebido es fallback si el preset no existe.

---

## Settings en fase 1

**Caso IRL:** reenviar un job migrado sin potencia/velocidad/pasadas/foco/kerf/tabs no reproduce el trabajo.

| Option | Description | Selected |
|--------|-------------|----------|
| Todos los de la operación | Snapshot completo; emisión en fase 3 | ✓ |
| Solo comunes ahora | Potencia/velocidad/pasadas | |
| Solo operación y orden | Settings en fase 3 | |
| Reusar GcodeSettings global | Settings globales | |

**User's choice:** Todos los de la operación.

---

## Documento nuevo y capas sin operación

**Caso IRL:** abres un SVG con capas sin operaciones y las asignas progresivamente.

| Option | Description | Selected |
|--------|-------------|----------|
| Staging/Unassigned | Base no produce; sus shapes no se exportan | Revisado |
| Capa normal opcional | `operation: Option`; None = no asignada | ✓ |
| Siempre staging | Base nunca produce | |
| Capa plantilla | Base es template/guide | |

**User's choice:** La operación es un marcador opcional de apariencia/producción, como Illustrator. Puede haber cualquier número de capas sin operación. Capa sin operación = no produce, no entra en secuencia; sus shapes son Unassigned. Capa con operación = pasada. Documento nuevo: una capa base sin operación.
**Notes:** Esto reemplaza la invariante anterior de "toda capa exige operación".

---

## Asignación de operación a capa

**Caso IRL:** una capa importada ya tiene shapes; asignas Cut y esperas que todo se corte.

| Option | Description | Selected |
|--------|-------------|----------|
| Todos los shapes pasan a la pasada | Asignar operación incluye el contenido | ✓ |
| Solo default para nuevos | Los existentes siguen unassigned | |
| Preguntar al asignar | Incluir todo o solo nuevos | |
| Mover a capa nueva | Crear capa y mover/copiar | |

**User's choice:** Todos los shapes pasan a la pasada.

---

## Orden intra-capa

**Caso IRL:** dentro de una capa de corte, primero agujeros interiores y luego contorno exterior; el orden afecta recorrido y estabilidad.

| Option | Description | Selected |
|--------|-------------|----------|
| Preservar orden, planner puede reordenar | `Vec<ShapeId>`; v1 conserva orden | ✓ |
| Sin orden; siempre optimizar | Conjunto sin orden | |
| Orden manual explícito ya | UI de orden en fase 1 | |
| Orden global del documento | Un orden para todas las capas | |

**User's choice:** Preservar orden, planner puede reordenar.

---

## the agent's Discretion

- Representación Rust de la operación (enum con datos vs `kind + settings`).
- Esquema exacto de v2 y serialización de memberships.
- Matriz de parámetros por proceso/estrategia.
- Mapeo exacto de `Operation`/`ToolOp`/`GcodeSettings` v1 a operaciones cocinadas.
- Formato de `preset_id` y versionado de profiles.

## Deferred Ideas

- UI de capas y catálogo "Nueva operación" — fase 2.
- UI de Assign/Link y gestión de profiles — fase 2.
- Emisión por operación — fase 3.
- 2-opt y reordenado manual intra-capa — fase 4.
- Streaming, recovery y raster — siguiente milestone.
