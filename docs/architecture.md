# Arquitectura de bide

> **Este árbol es el destino, no el punto de partida.**
> No se crean archivos vacíos por adelantado. Cada módulo nace detrás de un
> test rojo que lo justifica (TDD estricto, YAGNI). Este documento es el mapa
> de dónde acaba cada pieza cuando exista.

## Principio

bide no depende de un agente principal que lo haga todo. El núcleo es
**determinista** y está en Rust. Los agentes producen análisis, planes,
críticas o revisiones; **no controlan el sistema**. El control lo tiene el
Workflow Engine.

```
La IA razona.  Rust controla.  Lexis observa.  Signal valida.
Claude Code implementa.  Git registra.  bide decide mediante reglas.
```

```
Usuario → bide CLI → Workflow Engine → State Machine
                                          │
             ┌────────────────────────────┼────────────────────────────┐
        Context Builder             Agent Runner                  Tool Router
             │                           │                             │
           Lexis            Planner / Critic / Fixer / Reviewer   Signal · Claude Code · Git
```

## Estado actual

Implementado (con tests verdes):

- `WorkflowState` — 8 estados de la máquina.
- `StepOutcome` — `Success | Failure`.
- `Task` — `state` + `retries` + `max_retries`, con `advance(outcome)`.

Máquina:

```
Created → ContextReady → PlanReviewed → ChangesProduced → ChecksCompleted → Accepted
                                              ▲                  │ Failure
                                              └── FixPlanned ◀───┘  (retries < max)
                                                                  │ Failure & retries == max
   cualquier otro fallo ───────────────────────────────────────► Failed
```

## Árbol objetivo

```
bide/
├── Cargo.toml
├── rust-toolchain.toml
├── clippy.toml
├── justfile
├── bide.example.toml
├── README.md · LICENSE
│
├── docs/            architecture · workflow · tdd-strategy · tools · security
│
├── src/
│   ├── main.rs · lib.rs
│   ├── cli/         args · commands/{init,scan,plan,run,verify,review,status}
│   ├── core/        engine · workflow · state · transition · task · event
│   │                artifact · decision · retry · error
│   ├── context/     builder · repo_profile · file_selector · task_context · context_pack
│   ├── agents/      agent · runner · schemas · planner · critic · fix_planner · reviewer
│   ├── prompts/     templates + *.md
│   ├── tools/       tool · router · mcp_client · cli_tool · lexis · signal · claude_code · terminal
│   ├── verification/ runner · check · check_result · signal_adapter
│   ├── git/         manager · status · branch · diff · patch
│   ├── policy/      engine · command_policy · file_policy · secret_policy · approval
│   ├── memory/      store · project_memory · decision_memory · run_memory · artifact_store
│   ├── report/      final_report · markdown · summary
│   └── config/      loader · schema · defaults
│
├── tests/           tdd_*.rs + support/ (fakes) + fixtures/ + golden/
└── examples/
```

## Responsabilidades

| Pieza | Responsabilidad |
|---|---|
| Workflow Engine | Controla el proceso: ejecuta pasos, aplica reglas y límites, registra artefactos. |
| State Machine | Controla el estado. bide siempre sabe dónde está. |
| Context Builder | Prepara contexto del repo usando Lexis. Determinista. |
| Planner / Critic | Crea planes / los critica. Solo emiten salida estructurada. |
| Fix Planner | Ante un fallo, propone estrategia de reparación. No implementa. |
| Reviewer | Evalúa el diff final y recomienda aceptar / reintentar / rechazar. |
| Tool Router | Llama herramientas externas (MCP, CLIs), normaliza, aplica timeouts. |
| Verification Runner | Ejecuta checks con Signal. |
| Policy Engine | Protege el sistema. Fuera de los agentes. Zona intocable. |
| Git Manager | Diff, branch, patch, estado. Sin LLM. |
| Memory Store | Guarda historial en `.bide/`. No controla el flujo. |

## Regla de oro

Un agente puede decir *"recomiendo ejecutar cargo test"*, pero no lo ejecuta.
Eso lo decide el Workflow Engine y lo ejecuta el Tool Router.
