# Design and interface language

Start with the user's primary task, visible object, available actions, decision
information, empty/loading/error states and keyboard path. Preserve native
window, menu, pane, keyboard and focus conventions instead of importing browser
or mobile defaults into a desktop workflow.

Use semantic components before custom clickable containers. Components own
keyboard activation, focus, disabled state, accessible roles and dismissal.
Choose a table for comparison, a tree for hierarchy and a dock for arranging
long-lived tools; these are behavior choices rather than visual styles.

Read colors and radii from the active theme. Use relative sizing and spacing
helpers and semantic component sizes, so application zoom changes typography,
icons, control frames and gaps together. Direct pixels belong to documented
physical boundaries such as native insets, raster geometry or a placement probe.
Raw colors belong to theme definitions or data whose color is itself the content.

Keep a small hierarchy, shared alignment spines and consistent local density.
Place scrollbars at their scroll owner's edge and content inset inside that
owner. Release flex minimums on shrinkable children. Reserve space for focus
rings; clipping must not erase keyboard focus.

Use one clear default commit in a decision area. Primary styling is not a reward
for the only or most frequent button. Essential actions stay reachable without
hover. Put secondary commands in the appropriate menu and expose natural
keyboard actions. Use links for external resources and buttons/actions for
application operations. Selection, focus, hover and popup-open state are distinct.

## Interface language

Use nouns for destinations, verbs for commands and precise descriptions of
state. Let the containing context supply words it already establishes. Name the
scope and consequence when those affect a decision; do not expose backend
operations that have no visible object or understandable outcome.

Translate intent rather than word order. Preserve exact API identifiers and
established technical names when translation would reduce precision. Use natural
English and Chinese interface language, consistent terminology, sentence case
and an ellipsis when an action opens a further decision. Do not assemble sentences
from translated fragments. Keep accessible names aligned with visible intent.

Verify light/dark/custom themes, several window sizes, display scales and base
font sizes. Screenshots prove visual facts; semantic trees prove roles, labels,
values and actions. Neither alone establishes a complete interaction contract.
