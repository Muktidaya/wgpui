<p align="center">
  <strong>WGPUI Markdown regression input</strong>
  <br>
  <em>Structured text, code, links, tables, and inline HTML</em>
</p>

# Markdown document fixture

This is synthetic input for the WGPUI text-view regression suite. It provides a
substantial document whose parser can be rebuilt while the text remains stable.
The examples are formatting input, rather than installation instructions or a
record of publication and development operations.

The public project is [WGPUI](https://github.com/Muktidaya/wgpui). Its
[Kit guide](https://github.com/Muktidaya/wgpui/blob/root/docs/kit.md) describes
the application facade. This fixture intentionally mixes ordinary paragraphs,
**strong emphasis**, *italic emphasis*, `inline code`, and links in the same line.

## Package names and versions

The version values below identify the source candidate used in this document.
They exercise a table and TOML code fences; they do not assert publication or
completed native acceptance. The minimum Rust version is **1.95**.

| Package | Source candidate | Role |
| --- | --- | --- |
| `wgpui` | `0.4.0` | Native framework |
| `wgpui_derive` | `0.4.0` | Framework procedural macros |
| `wgpui-http-client` | `0.1.0` | HTTP interfaces and asynchronous bodies |
| `wgpui-kit` | `0.8.1` | Application facade |
| `wgpui-base` | `0.8.1` | Behavior and infrastructure |
| `wgpui-component` | `0.8.1` | Styled controls |
| `wgpui-kit-assets` | `0.8.1` | Icon catalog |
| `wgpui-kit-macros` | `0.8.1` | Component macros |
| `wgpui-platform` | `0.8.1` | Platform entry |
| `wgpui-fps` | `0.8.1` | Performance presentation |

```toml
[package]
name = "markdown-fixture"
version = "0.0.0"
edition = "2024"
rust-version = "1.95"
publish = false

[dependencies]
wgpui = "0.4.0"
wgpui-kit = "0.8.1"
```

The facade combines framework types with Base behavior and styled controls.
These independently versioned packages deliberately share one framework type
identity. A paragraph following a code block must return to ordinary typography
and continue wrapping within the available width.

## A layered document

- **Framework:** windows, entities, actions, input, text, and drawing.
- **Base:** reusable interaction and application-owned presentation.
- **Components:** semantic themes and styled controls.
- **Assets:** selected or default icons.
- **Application:** domain state, navigation, and task-specific composition.

The list continues with nested content to exercise indentation and marker layout:

1. Construct the document's state.
   - Keep the entity handle for later updates.
   - Give interactive elements stable identifiers.
   - Put callbacks after the context argument.
2. Render the current value.
   - Compose a label, an input, and an action.
   - Keep disabled and selected state explicit.
3. Observe a state transition.
   - Update the entity through its inner context.
   - Notify after changing visible state.
4. Rebuild the same parser without changing this input.

> A stable document can be parsed again without scheduling an endless sequence
> of redraws. This block quotation supplies multiple wrapped lines, including
> **emphasis** and `code`, while remaining part of the same long-form input.
>
> A second paragraph preserves the distinction between an empty line and a hard
> line break inside a quoted block.

### Rust source as text

The following fenced block includes generic types, lifetimes, references,
closures, a method chain, and string literals. The regression reads the source
as Markdown; it does not compile or execute the example.

```rust
use wgpui_kit::*;

actions!(fixture, [Confirm]);

struct Document {
    title: SharedString,
    confirmed: bool,
}

impl Render for Document {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("document")
            .flex()
            .flex_col()
            .gap_2()
            .child(self.title.clone())
            .child(
                div()
                    .id("confirm")
                    .child("Confirm")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirmed = true;
                        cx.notify();
                    })),
            )
            .when(self.confirmed, |this| this.child("Confirmed"))
    }
}

fn describe<'a>(name: &'a str, values: &[Option<usize>]) -> (&'a str, usize) {
    (name, values.iter().flatten().count())
}
```

After this longer code block, the next paragraph deliberately has enough words
to wrap across several lines at ordinary window sizes. Its clauses are distinct
and its punctuation varies: commas separate related values, parentheses provide
extra context, and a colon introduces a concrete list of colors, shapes, and
measurements. Repeated rendering must retain the content without repeating the
document's construction indefinitely.

### Mixed inline HTML

<div>
  <p>A paragraph with <strong>strong HTML text</strong>, <em>emphasis</em>, and
  a <a href="https://github.com/Muktidaya/wgpui" title="WGPUI public project">public project link</a>.</p>
  <p>Literal entities remain text: &lt;Window&gt;, &amp;, &quot;label&quot; and &#x1F980;.</p>
</div>

<details>
  <summary>Additional document material</summary>
  <p>This nested HTML block supplies another paragraph and <code>inline code</code>.</p>
</details>

The text returns to Markdown after the HTML block. Keyboard labels such as
<kbd>Ctrl</kbd> + <kbd>Enter</kbd> appear beside ordinary inline text, while
escaped markup such as \<literal\> remains available to the parser.

## Layout vocabulary

| Kind | Example | Meaning in this fixture |
| :--- | :---: | ---: |
| Identifier | `document-title` | Stable element identity |
| Length | `24px` | A short numeric label |
| Percentage | `100%` | A value containing punctuation |
| Range | `0..12` | Byte-oriented editor notation |
| Character | `Δ` | A non-ASCII scalar |
| Text | `日本語` | CJK content |
| Emoji | `🦀` | Supplementary-plane content |

Text may include mixed scripts: Latin, Ελληνικά, 日本語, العربية, and emoji 🦀.
Those sequences exercise shaping and fallback without making a particular font
file part of the fixture. The application can select an explicit family; the
framework's system-role names are `.SystemUIFont` and `.SystemMonospaceFont`.

### Further paragraphs for wrapping

A text view can display an introductory paragraph, a short heading, a long code
block, a table, and a trailing explanation in one document. Each block has its
own size, yet the document should settle after the parser and layout agree.
This paragraph mentions ordinary interface tasks: editing a name, reviewing a
table, opening a disclosure, choosing an item, and confirming a value.

Another paragraph keeps the content long enough to span more than a trivial
sample. It includes a link to the [framework documentation](https://docs.rs/wgpui),
two **separate bold phrases**, and an inline expression such as `Some("text")`.
The link is content, the code is content, and neither one is an instruction to
contact a service or alter state during the regression.

- A trailing list retains its marker after the preceding paragraph.
- A second item includes *emphasis* and `0.8.1`.
- A third item links to [the public WGPUI source](https://github.com/Muktidaya/wgpui).

---

## Closing block

This final section supplies a heading and a paragraph after a thematic break.
The document contains multiple headings, long text, nested lists, aligned
tables, Rust and TOML fences, block quotations, links, inline HTML, entities,
and Unicode. Rebuilding its parser should preserve its meaning and reach a
stable rendered frame.
