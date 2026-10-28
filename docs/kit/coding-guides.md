# Coding and lifecycle

Read [the design guide](design-guides.md) before changing a product flow. Keep
feature state, commands, views and workflow together; extract shared code only
when it has a coherent reusable contract. Dependencies point toward smaller
behavior and framework layers, never from Base back into Component or an app.

Use `RenderOnce` for value-like composition and `Entity<T>`/`Render` when state,
observation, focus, history or asynchronous work survives frames. `Context<T>`
updates that entity, `App` coordinates application state, and `Window` owns its
focus, actions, measurement and input. Retain handles rather than borrowed
contexts. Use the context supplied to each update closure.

Keep render callbacks declarative and side-effect-free. Update related fields
coherently and notify their owner once. Store or detach asynchronous tasks and
subscriptions according to their intended lifetime. Propagate failures or log a
handled failure; silently dropping a failed operation is not recovery.

Assign stable domain-based element IDs. Reordering, translation and mutable
labels must not change identity accidentally. Use explicit component parts and
callbacks rather than discovering semantic roles by inspecting descendants.

Use WGPUI actions and focus handles for commands. Input callbacks may reenter
view updates, replace handlers or rebuild menus: release storage borrows before
invoking them and preserve replacements installed by nested work. Distinguish a
requested change from synchronizing an owner-supplied value.

Initialize Kit once and open each window through its facade. Use the [current
application example](../../examples/kit-hello-world/README.md) for compilable
bootstrap code. Internal dependency aliases and public `AppContext` traits are
compatibility contracts; obsolete `WindowContext`, `ViewContext` and `Model<T>`
examples are not templates for new code.

Expose deliberate public seams with valid defaults, builders and readers.
Document callback ordering, focus ownership, errors, platform capabilities and
state lifetime. Preserve established API names while improving internals.
Use current Rust signatures and the workspace's formatter and strict lint wrapper.
See [testing](testing.md) for behavioral acceptance and [state styles](styling-and-motion.md)
for presentation ownership.
