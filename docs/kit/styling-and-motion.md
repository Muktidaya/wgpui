# State styles and motion

The framework owns runtime hover, active, focus and focus-visible refinements.
Base owns reusable semantic-state resolution and transition lifecycle. Component
or the application supplies target styles, variants, timing and animated values.
Controlled values such as checked, selected and open remain owned by callers.

Keep state-independent geometry in the ordinary builder chain. Apply semantic
checked/selected/pressed/disabled styles through the component's public seam.
Later refinements replace only the properties they set; do not register the same
runtime pseudo-state twice or inspect private descendant styles to merge them.
Guard hover and active treatments when disabled controls must not respond.

Use semantic theme values. After directly changing Component's global theme,
call `Theme::sync_base` so Base-owned elements receive the projection. A complete
`Theme::change` performs its own projection. Product-owned spacing/elevation
scales must have an explicit owner rather than assuming every custom token is
stored by the global theme.

Motion explains appearance, dismissal and spatial continuity. Give independent
transition channels stable identities, support interruption and reduced motion,
and keep animation work bounded. A measurement or cached layout must invalidate
when its width, content, scale or base font changes. A test-clock advance is not
a substitute for a wall-clock animation's completion.

For overflow, distinguish the layout viewport from its clipping mask. A child in
a max-height flex panel needs a shrinkable flex chain; clipping alone does not
create a scroll viewport. Keep the focus indicator and scrollbar inside their
actual owners. See [the application guide](../kit.md#source-candidate-compatibility)
for the current Taffy behavior.
