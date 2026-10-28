use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

use html5ever::{
    Attribute, ExpandedName, QualName,
    tendril::StrTendril,
    tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink},
};

type Handle = Rc<Node>;

pub(super) enum NodeData {
    Document,
    Doctype,
    Text {
        contents: RefCell<StrTendril>,
    },
    Comment {
        contents: StrTendril,
    },
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,
        template_contents: Option<Handle>,
        mathml_annotation_xml_integration_point: bool,
    },
    ProcessingInstruction,
}

pub(super) struct Node {
    parent: RefCell<Weak<Node>>,
    pub(super) children: RefCell<Vec<Handle>>,
    pub(super) data: NodeData,
}

impl Node {
    fn new(data: NodeData) -> Handle {
        Rc::new(Self {
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(Vec::new()),
            data,
        })
    }

    fn take_children(&mut self, pending: &mut Vec<Handle>) {
        pending.append(self.children.get_mut());
        if let NodeData::Element {
            template_contents, ..
        } = &mut self.data
            && let Some(contents) = template_contents.take()
        {
            pending.push(contents);
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        // Deep HTML must not recurse through Rc drops, or drain subtrees retained elsewhere.
        let mut pending = Vec::new();
        self.take_children(&mut pending);
        while let Some(node) = pending.pop() {
            if let Ok(mut node) = Rc::try_unwrap(node) {
                node.take_children(&mut pending);
            }
        }
    }
}

pub(super) struct HtmlDom {
    pub(super) document: Handle,
    errors: RefCell<Vec<Cow<'static, str>>>,
    quirks_mode: Cell<QuirksMode>,
}

impl Default for HtmlDom {
    fn default() -> Self {
        Self {
            document: Node::new(NodeData::Document),
            errors: RefCell::new(Vec::new()),
            quirks_mode: Cell::new(QuirksMode::NoQuirks),
        }
    }
}

fn detach(node: &Handle) {
    let parent = node.parent.replace(Weak::new()).upgrade();
    if let Some(parent) = parent {
        let mut children = parent.children.borrow_mut();
        // Parent links are written only together with insertion into its child list.
        let index = children
            .iter()
            .position(|child| Rc::ptr_eq(child, node))
            .expect("HTML node's parent must contain the node");
        children.remove(index);
    }
}

fn append_node(parent: &Handle, node: Handle) {
    detach(&node);
    node.parent.replace(Rc::downgrade(parent));
    parent.children.borrow_mut().push(node);
}

fn append_text(node: &Handle, text: &str) -> bool {
    if let NodeData::Text { contents } = &node.data {
        contents.borrow_mut().push_slice(text);
        true
    } else {
        false
    }
}

impl TreeSink for HtmlDom {
    type Handle = Handle;
    type Output = Self;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> Self::Output {
        self
    }

    fn parse_error(&self, message: Cow<'static, str>) {
        self.errors.borrow_mut().push(message);
    }

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> Self::ElemName<'a> {
        match &target.data {
            NodeData::Element { name, .. } => name.expanded(),
            _ => unreachable!("html5ever requests element names only for elements"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents: flags.template.then(|| Node::new(NodeData::Document)),
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn create_comment(&self, contents: StrTendril) -> Handle {
        Node::new(NodeData::Comment { contents })
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        Node::new(NodeData::ProcessingInstruction)
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        let node = match child {
            NodeOrText::AppendNode(node) => node,
            NodeOrText::AppendText(text) => {
                if let Some(previous) = parent.children.borrow().last()
                    && append_text(previous, &text)
                {
                    return;
                }
                Node::new(NodeData::Text {
                    contents: RefCell::new(text),
                })
            }
        };
        append_node(parent, node);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        previous_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if element.parent.borrow().upgrade().is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(previous_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
        append_node(&self.document, Node::new(NodeData::Doctype));
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        match &target.data {
            NodeData::Element {
                template_contents: Some(contents),
                ..
            } => contents.clone(),
            _ => unreachable!("html5ever requests template contents only for templates"),
        }
    }

    fn same_node(&self, first: &Handle, second: &Handle) -> bool {
        Rc::ptr_eq(first, second)
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        if let NodeOrText::AppendNode(node) = &child {
            detach(node);
        }
        // The tree builder guarantees the insertion sibling has a parent.
        let parent = sibling
            .parent
            .borrow()
            .upgrade()
            .expect("HTML insertion sibling must have a parent");
        let mut children = parent.children.borrow_mut();
        let index = children
            .iter()
            .position(|node| Rc::ptr_eq(node, sibling))
            .expect("HTML insertion sibling must belong to its parent");
        let node = match child {
            NodeOrText::AppendNode(node) => node,
            NodeOrText::AppendText(text) => {
                if let Some(previous) = index.checked_sub(1).and_then(|index| children.get(index))
                    && append_text(previous, &text)
                {
                    return;
                }
                Node::new(NodeData::Text {
                    contents: RefCell::new(text),
                })
            }
        };
        node.parent.replace(Rc::downgrade(&parent));
        children.insert(index, node);
    }

    fn add_attrs_if_missing(&self, target: &Handle, attributes: Vec<Attribute>) {
        let NodeData::Element { attrs, .. } = &target.data else {
            unreachable!("html5ever adds attributes only to elements");
        };
        let mut existing = attrs.borrow_mut();
        for attribute in attributes {
            if !existing.iter().any(|value| value.name == attribute.name) {
                existing.push(attribute);
            }
        }
    }

    fn remove_from_parent(&self, target: &Handle) {
        detach(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let children = node.children.take();
        for child in &children {
            child.parent.replace(Rc::downgrade(new_parent));
        }
        new_parent.children.borrow_mut().extend(children);
    }

    fn is_mathml_annotation_xml_integration_point(&self, target: &Handle) -> bool {
        matches!(
            target.data,
            NodeData::Element {
                mathml_annotation_xml_integration_point: true,
                ..
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use html5ever::{parse_document, tendril::TendrilSink};

    fn parse(source: &str) -> HtmlDom {
        parse_document(HtmlDom::default(), Default::default()).one(source)
    }

    fn find(node: &Handle, local: &str) -> Option<Handle> {
        if let NodeData::Element { name, .. } = &node.data
            && &*name.local == local
        {
            return Some(node.clone());
        }
        node.children
            .borrow()
            .iter()
            .find_map(|child| find(child, local))
    }

    fn children(node: &Handle) -> Vec<String> {
        node.children
            .borrow()
            .iter()
            .map(|child| match &child.data {
                NodeData::Element { name, .. } => format!("<{}>", name.local),
                NodeData::Text { contents } => contents.borrow().to_string(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn repairs_misnested_formatting() {
        let dom = parse("<p><b>one<i>two</b>three</i>four");
        let paragraph = find(&dom.document, "p").expect("parsed paragraph");
        assert_eq!(children(&paragraph), ["<b>", "<i>", "four"]);
        let bold = find(&paragraph, "b").expect("bold node");
        assert_eq!(children(&bold), ["one", "<i>"]);
        let italic = paragraph
            .children
            .borrow()
            .get(1)
            .cloned()
            .expect("repaired italic node");
        assert_eq!(children(&italic), ["three"]);
    }

    #[test]
    fn foster_parents_table_text_and_inserts_table_sections() {
        let dom = parse("<div><table>before<tr><td>A&amp;B<td>𝄞</table>after</div>");
        let container = find(&dom.document, "div").expect("outer container");
        assert_eq!(children(&container), ["before", "<table>", "after"]);
        let table = find(&container, "table").expect("table");
        assert_eq!(children(&table), ["<tbody>"]);
        let row = find(&table, "tr").expect("table row");
        assert_eq!(children(&row), ["<td>", "<td>"]);
        let cells = row.children.borrow();
        assert_eq!(children(cells.first().expect("first cell")), ["A&B"]);
        assert_eq!(children(cells.get(1).expect("second cell")), ["𝄞"]);
    }

    #[test]
    fn merges_missing_body_attributes_and_preserves_foreign_names() {
        let dom = parse(
            r#"<body class=first><body id=late class=second><svg viewBox="0 0 10 10"><foreignObject><p data-x="a&amp;&quot;b">text</p></foreignObject></svg>"#,
        );
        let body = find(&dom.document, "body").expect("body");
        let NodeData::Element { attrs, .. } = &body.data else {
            panic!("body must be an element");
        };
        let attributes: Vec<_> = attrs
            .borrow()
            .iter()
            .map(|attribute| {
                (
                    attribute.name.local.to_string(),
                    attribute.value.to_string(),
                )
            })
            .collect();
        assert_eq!(
            attributes,
            [
                ("class".into(), "first".into()),
                ("id".into(), "late".into())
            ]
        );
        let svg = find(&body, "svg").expect("SVG element");
        let NodeData::Element { name, attrs, .. } = &svg.data else {
            panic!("SVG must be an element");
        };
        assert_eq!(&*name.ns, "http://www.w3.org/2000/svg");
        assert_eq!(
            &*attrs
                .borrow()
                .first()
                .expect("viewBox attribute")
                .name
                .local,
            "viewBox"
        );
        let paragraph = find(&svg, "p").expect("foreign-object HTML paragraph");
        let NodeData::Element { name, attrs, .. } = &paragraph.data else {
            panic!("paragraph must be an element");
        };
        assert_eq!(&*name.ns, "http://www.w3.org/1999/xhtml");
        assert_eq!(
            attrs
                .borrow()
                .first()
                .expect("data attribute")
                .value
                .as_ref(),
            "a&\"b"
        );
    }

    #[test]
    fn retains_inert_template_fragments_and_mathml_html_integration() {
        let dom = parse(
            r#"<template><p>inert</p></template><math><annotation-xml encoding="text/html"><p>visible</p></annotation-xml></math>"#,
        );
        let template = find(&dom.document, "template").expect("template");
        assert!(template.children.borrow().is_empty());
        let fragment = dom.get_template_contents(&template);
        let paragraph = find(&fragment, "p").expect("template paragraph");
        assert_eq!(children(&paragraph), ["inert"]);
        let integration = find(&dom.document, "annotation-xml").expect("MathML integration point");
        assert!(dom.is_mathml_annotation_xml_integration_point(&integration));
        let paragraph = find(&integration, "p").expect("integrated HTML paragraph");
        let NodeData::Element { name, .. } = &paragraph.data else {
            panic!("paragraph must be an element");
        };
        assert_eq!(&*name.ns, "http://www.w3.org/1999/xhtml");
    }

    #[test]
    fn releasing_document_preserves_a_retained_subtree() {
        let dom = parse("<div><span>retained</span></div>");
        let retained = find(&dom.document, "div").expect("retained subtree");
        drop(dom);
        let span = find(&retained, "span").expect("descendant survives document drop");
        assert_eq!(children(&span), ["retained"]);
    }

    #[test]
    fn drops_deep_trees_without_recursing() -> Result<(), Box<dyn std::error::Error>> {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let root = Node::new(NodeData::Document);
                let mut parent = root.clone();
                for _ in 0..20_000 {
                    let child = Node::new(NodeData::Document);
                    append_node(&parent, child.clone());
                    parent = child;
                }
                drop(parent);
                drop(root);
            })?
            .join()
            .map_err(|_| std::io::Error::other("deep HTML tree drop failed"))?;
        Ok(())
    }
}
