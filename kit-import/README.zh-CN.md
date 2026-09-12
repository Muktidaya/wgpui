<p align="center">
  <img src="./website/public/logo.svg" width="112" alt="GPUI Component logo" />
  <br>
  <strong>wgpui-component</strong>
</p>

[English](./README.md) | [简体中文](./README.zh-CN.md)

这是 Longbridge [gpui-component](https://github.com/longbridge/gpui-component) **0.6.0** 到独立版本 **[wgpui](https://github.com/Muktidaya/wgpui) 0.3.5** 的 **适配层**。

两个版本号是刻意分开的：本 crate 跟踪上游组件 **0.6.0**；运行时是 **wgpui 0.3.5**，不是 Zed 的 `gpui`，也不是 crates.io 上的 `gpui-pre`。发布名是 `wgpui-*`。本仓库**不会**以 `gpui-kit` 的名字发布。详见 [STATUS.md](./STATUS.md)。

```toml
[dependencies]
wgpui = "0.3.5"
wgpui-component = "0.6.0"
```

crates.io 消费方使用 `wgpui` 与 `wgpui_component`。本工作区把 `gpui` / `gpui_component` 别名到这些包，以便上游源码无需改 import。

## 特性

- **60+ 组件**：表单、导航、浮层、反馈、布局等。
- **生产来源**：上游 GPUI Kit 用于 Longbridge Pro。
- **原生手感**：macOS / Windows 风格控件，语义化主题。
- **GPU UI**：通过 wgpui（wgpu + winit）绘制。
- **数据表格 / 虚拟列表 / 代码编辑器 / Dock / Markdown / HTML / 图表**。
- **设计分层**：完整视觉系统用 `wgpui-component`；自建设计系统用 `wgpui-base`。
- **跨平台**：macOS、Windows、Linux。

## Crate 对应

| 发布名 | 路径 | 上游名 |
| --- | --- | --- |
| `wgpui-component` | `crates/ui` | `gpui-component` |
| `wgpui-base` | `crates/base` | `gpui-base` |
| `wgpui-component-macros` | `crates/macros` | `gpui-component-macros` |
| `wgpui-component-assets` | `crates/assets` | `gpui-kit-assets` |
| `wgpui-platform`（`publish = false`） | `crates/platform` | `gpui_platform` 入口垫片 |

`gpui-kit`、Story 画廊、WASM、`gpui-wry` 仍在磁盘上，但不是本工作区成员。`gpui-shell` 此处不编译。

[架构说明 →](docs/ARCHITECTURE.md)

## 使用

```toml
[dependencies]
wgpui = "0.3.5"
wgpui-component = "0.6.0"
```

在 `wgpui` 0.3.5 进入 crates.io 之前，请用 git 或 path 依赖。本 0.6.0 线尚未上传到 crates.io。

### 基础示例

```rs
use wgpui::*;
use wgpui_component::{button::*, *};

fn main() {
    wgpui::Application::new().run(move |cx| {
        wgpui_component::init(cx);
        // ...
    });
}
```

仓库内 [`examples/hello_world`](examples/hello_world) 使用同样的公开 crate 名。

## 开发

默认分支是 **`root`**。

```bash
git clone https://github.com/Muktidaya/wgpui-component.git
```

需要同级的 [wgpui](https://github.com/Muktidaya/wgpui)（`../wgpui`，分支 `root`）。工具链：Rust **1.94.0**。

```bash
cargo +1.94.0 run -p hello_world
cargo +1.94.0 test -p wgpui-component --lib
```

上游 Story 画廊不是工作区成员；不带 `-p` 的 `cargo run` 跑的是 `hello_world`。

## 许可证

Apache-2.0。上游版权仍归 Longbridge，见 [LICENSE-APACHE](./LICENSE-APACHE)。运行时是 [wgpui](https://github.com/Muktidaya/wgpui)。
