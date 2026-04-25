use obscura_dom::tree::{DomTree, NodeId};
use serde_json::{json, Value};

use crate::dispatch::CdpContext;

/// Build a simplified accessibility tree from the DOM.
/// This provides the minimal AXTree needed by agent-browser / Playwright
/// for ariaSnapshot generation.
fn build_ax_tree(dom: &DomTree, node_id: NodeId, depth: usize) -> Value {
    let node = match dom.get_node(node_id) {
        Some(n) => n,
        None => return json!({}),
    };

    let role = infer_aria_role(dom, node_id);
    let name = get_accessible_name(dom, node_id, &role);

    let mut children: Vec<Value> = Vec::new();
    let child_ids = dom.children(node_id);
    for child_id in child_ids {
        let child_role = infer_aria_role(dom, child_id);
        // Only include elements that have meaningful roles or are structural
        if !child_role.is_empty() || depth == 0 {
            children.push(build_ax_tree(dom, child_id, depth + 1));
        }
    }

    let mut ax_node = json!({
        "role": role,
        "name": name,
    });

    // Add properties based on role
    add_ax_properties(&mut ax_node, dom, node_id, &role);

    if !children.is_empty() || depth == 0 {
        ax_node["children"] = json!(children);
    }

    ax_node
}

fn infer_aria_role(dom: &DomTree, node_id: NodeId) -> String {
    let node = match dom.get_node(node_id) {
        Some(n) => n,
        None => return String::new(),
    };

    let tag_name = match node.as_element() {
        Some(qualname) => qualname.local.as_ref().to_lowercase(),
        None => return String::new(),
    };

    match tag_name.as_str() {
        "a" => "link".to_string(),
        "button" => "button".to_string(),
        "input" => {
            let input_type = node
                .get_attribute("type")
                .unwrap_or_default()
                .to_lowercase();
            match input_type.as_str() {
                "checkbox" => "checkbox".to_string(),
                "radio" => "radio".to_string(),
                "submit" | "button" => "button".to_string(),
                "search" => "searchbox".to_string(),
                _ => "textbox".to_string(),
            }
        }
        "textarea" => "textbox".to_string(),
        "select" => {
            let has_multiple = node.get_attribute("multiple").is_some();
            if has_multiple {
                "listbox".to_string()
            } else {
                "combobox".to_string()
            }
        }
        "nav" => "navigation".to_string(),
        "header" => "banner".to_string(),
        "main" | "body" => "main".to_string(),
        "aside" => "complementary".to_string(),
        "footer" => "contentinfo".to_string(),
        "article" => "article".to_string(),
        "section" => "region".to_string(),
        "form" => "form".to_string(),
        "dialog" => "dialog".to_string(),
        "img" | "image" => "img".to_string(),
        "h1" => "heading".to_string(),
        "h2" => "heading".to_string(),
        "h3" => "heading".to_string(),
        "h4" => "heading".to_string(),
        "h5" => "heading".to_string(),
        "h6" => "heading".to_string(),
        "ul" | "ol" => "list".to_string(),
        "li" => "listitem".to_string(),
        "table" => "table".to_string(),
        "thead" | "tbody" | "tfoot" => "rowgroup".to_string(),
        "tr" => "row".to_string(),
        "td" | "th" => "cell".to_string(),
        "span" | "div" => {
            if let Some(role) = node.get_attribute("role") {
                role.to_lowercase()
            } else {
                "generic".to_string()
            }
        }
        "p" => "paragraph".to_string(),
        "label" => "label".to_string(),
        "fieldset" => "group".to_string(),
        "legend" => "description".to_string(),
        "figure" => "figure".to_string(),
        "figcaption" => "caption".to_string(),
        "details" => "details".to_string(),
        "summary" => "summary".to_string(),
        "menu" => "menu".to_string(),
        "menuitem" => "menuitem".to_string(),
        "option" => "option".to_string(),
        "progressbar" => "progressbar".to_string(),
        "meter" => "meter".to_string(),
        "switch" => "switch".to_string(),
        "iframe" => "iframe".to_string(),
        "embed" | "object" => "embed".to_string(),
        "canvas" => "canvas".to_string(),
        "video" | "audio" => "media".to_string(),
        "script" | "style" | "link" | "meta" => String::new(), // non-visible
        _ => String::new(),
    }
}

fn get_accessible_name(dom: &DomTree, node_id: NodeId, role: &str) -> String {
    let node = match dom.get_node(node_id) {
        Some(n) => n,
        None => return String::new(),
    };

    // Check aria-label first
    if let Some(label) = node.get_attribute("aria-label") {
        if !label.is_empty() {
            return label.to_string();
        }
    }

    // Check aria-labelledby
    if let Some(labelledby) = node.get_attribute("aria-labelledby") {
        if !labelledby.is_empty() {
            // Try to resolve by ID attribute - look for element with matching id
            let parts: Vec<String> = labelledby
                .split_whitespace()
                .filter_map(|id| dom.get_element_by_id(id).map(|nid| dom.text_content(nid)))
                .collect();
            if !parts.is_empty() {
                return parts.join(" ");
            }
        }
    }

    // For certain roles, prefer text content
    match role {
        "link" | "button" | "heading" | "label" | "menuitem" | "option" | "paragraph"
        | "caption" | "summary" | "listitem" => {
            let text = dom.text_content(node_id);
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
        _ => {}
    }

    // For inputs, prefer placeholder
    if role == "textbox" || role == "searchbox" {
        if let Some(placeholder) = node.get_attribute("placeholder") {
            if !placeholder.is_empty() {
                return placeholder.to_string();
            }
        }
    }

    String::new()
}

fn add_ax_properties(ax_node: &mut Value, dom: &DomTree, node_id: NodeId, role: &str) {
    let node = match dom.get_node(node_id) {
        Some(n) => n,
        None => return,
    };

    let mut props: Vec<Value> = Vec::new();

    // Add disabled state
    if node.get_attribute("disabled").is_some()
        || node
            .get_attribute("aria-disabled")
            .map(|v| v == "true")
            .unwrap_or(false)
    {
        props.push(json!({
            "name": "disabled",
            "value": {"type": "boolean", "value": true}
        }));
    }

    // Add readonly state
    if node.get_attribute("readonly").is_some()
        || node
            .get_attribute("aria-readonly")
            .map(|v| v == "true")
            .unwrap_or(false)
    {
        props.push(json!({
            "name": "readonly",
            "value": {"type": "boolean", "value": true}
        }));
    }

    // Add expanded state for collapsible elements
    if role == "details" || role == "combobox" {
        let expanded = node.get_attribute("open").is_some();
        props.push(json!({
            "name": "expanded",
            "value": {"type": "boolean", "value": expanded}
        }));
    }

    // Add selected state for options
    if role == "option" {
        let selected = node.get_attribute("selected").is_some();
        props.push(json!({
            "name": "selected",
            "value": {"type": "boolean", "value": selected}
        }));
    }

    // Add checked state for checkboxes/radios/switches
    if role == "checkbox" || role == "radio" || role == "switch" {
        let checked = node.get_attribute("checked").is_some();
        props.push(json!({
            "name": "checked",
            "value": {"type": "boolean", "value": checked}
        }));
    }

    // Add description via aria-describedby
    if let Some(describedby) = node.get_attribute("aria-describedby") {
        if !describedby.is_empty() {
            let parts: Vec<String> = describedby
                .split_whitespace()
                .filter_map(|id| {
                    dom.get_element_by_id(id)
                        .and_then(|nid| Some(dom.text_content(nid)))
                })
                .collect();
            if !parts.is_empty() {
                props.push(json!({
                    "name": "description",
                    "value": {"type": "string", "value": parts.join(" ")}
                }));
            }
        }
    }

    // Add level for headings
    if role == "heading" {
        let level = node
            .as_element()
            .and_then(|q| q.local.as_ref().strip_prefix('H'))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        props.push(json!({
            "name": "level",
            "value": {"type": "number", "value": level}
        }));
    }

    // Add required state for form controls
    if role == "textbox" || role == "combobox" || role == "checkbox" || role == "radio" {
        if node.get_attribute("required").is_some()
            || node
                .get_attribute("aria-required")
                .map(|v| v == "true")
                .unwrap_or(false)
        {
            props.push(json!({
                "name": "required",
                "value": {"type": "boolean", "value": true}
            }));
        }
    }

    // Add invalid state
    if node
        .get_attribute("aria-invalid")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        props.push(json!({
            "name": "invalid",
            "value": {"type": "boolean", "value": true}
        }));
    }

    if !props.is_empty() {
        ax_node["properties"] = json!(props);
    }
}

pub async fn handle(
    method: &str,
    params: &Value,
    ctx: &mut CdpContext,
    session_id: &Option<String>,
) -> Result<Value, String> {
    match method {
        "getFullAXTree" => {
            let page = ctx
                .get_session_page(session_id)
                .ok_or("No page for session")?;

            let dom = match &page.dom {
                Some(d) => d,
                None => return Ok(json!({"nodes": []})),
            };

            // Find root - use body or html as starting point
            let root_id = dom.find_body_or_root();

            let ax_tree = build_ax_tree(dom, root_id, 0);

            // Return as agent-browser/Playwright expects: { nodes: [...] }
            Ok(json!({
                "nodes": [ax_tree]
            }))
        }
        "getPartialAXTree" => {
            // Get node ID to query
            let node_id = params.get("nodeId").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

            let page = ctx
                .get_session_page(session_id)
                .ok_or("No page for session")?;
            let dom = match &page.dom {
                Some(d) => d,
                None => return Ok(json!({"nodes": []})),
            };

            let ax_node = build_ax_tree(dom, NodeId::new(node_id), 0);

            Ok(json!({
                "nodes": [ax_node]
            }))
        }
        "getAXNodeAndAncestors" => {
            // Get node ID to query
            let node_id = params.get("nodeId").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

            let page = ctx
                .get_session_page(session_id)
                .ok_or("No page for session")?;
            let dom = match &page.dom {
                Some(d) => d,
                None => return Ok(json!({"nodes": []})),
            };

            // Build list of ancestors
            let mut nodes: Vec<Value> = Vec::new();
            let ancestors = dom.ancestors(NodeId::new(node_id));
            for ancestor_id in ancestors {
                nodes.push(build_ax_tree(dom, ancestor_id, 0));
            }
            // Also add the node itself
            nodes.push(build_ax_tree(dom, NodeId::new(node_id), 0));

            Ok(json!({"nodes": nodes}))
        }
        "enable" => {
            // Accessibility is always enabled in our implementation
            Ok(json!({}))
        }
        "disable" => {
            // No-op
            Ok(json!({}))
        }
        _ => Err(format!("Unknown Accessibility method: {}", method)),
    }
}
