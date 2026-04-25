//! Style module - computes styles from inline styles, external stylesheets, and element attributes

use once_cell::sync::Lazy;
use regex::Regex;

use obscura_dom::tree::{DomTree, NodeId};

/// A parsed CSS stylesheet
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    /// All CSS rules mapped by selector to declarations
    rules: Vec<CSSRule>,
}

/// A single CSS rule with selector and declarations
#[derive(Debug, Clone)]
pub struct CSSRule {
    pub selector: String,
    pub declarations: Vec<CSSDeclaration>,
}

/// A CSS declaration (property: value)
#[derive(Debug, Clone)]
pub struct CSSDeclaration {
    pub property: String,
    pub value: String,
}

/// Flex direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl Default for FlexDirection {
    fn default() -> Self {
        FlexDirection::Row
    }
}

/// Flex wrap
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

impl Default for FlexWrap {
    fn default() -> Self {
        FlexWrap::NoWrap
    }
}

/// Justify content for flexbox
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl Default for JustifyContent {
    fn default() -> Self {
        JustifyContent::FlexStart
    }
}

/// Align items for flexbox
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

impl Default for AlignItems {
    fn default() -> Self {
        AlignItems::Stretch
    }
}

/// Position mode for an element
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
}

impl Default for Position {
    fn default() -> Self {
        Position::Static
    }
}

/// Computed styles for an element
#[derive(Debug, Clone, Default)]
pub struct ComputedStyles {
    pub width: String,
    pub height: String,
    pub padding: String,
    pub margin: String,
    pub background_color: Option<[u8; 4]>,
    pub color: Option<[u8; 4]>,
    pub font_size: String,
    pub font_family: String,
    pub display: String,
    pub position: Position,
    pub border: String,
    // Position offsets
    pub top: String,
    pub right: String,
    pub bottom: String,
    pub left: String,
    // z-index for stacking
    pub z_index: i32,
    // Flexbox properties
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: String,
    pub align_self: Option<AlignItems>,
    pub gap: String,
    // Grid properties
    pub grid_template_columns: String,
    pub grid_template_rows: String,
    pub grid_gap: String,
    // Box sizing
    pub box_sizing: String,
}

static SELECTOR_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"#([a-zA-Z0-9_-]+)").unwrap());
static SELECTOR_CLASS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.([a-zA-Z0-9_-]+)").unwrap());
static SELECTOR_TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([a-zA-Z0-9]+)").unwrap());

impl Stylesheet {
    /// Parse a CSS stylesheet from a string
    pub fn parse(css: &str) -> Self {
        let mut rules = Vec::new();
        let mut current_pos = 0;
        let css_bytes = css.as_bytes();

        while current_pos < css_bytes.len() {
            // Skip whitespace and comments
            let (pos, _) = Self::skip_whitespace_and_comments(css_bytes, current_pos);
            current_pos = pos;

            if current_pos >= css_bytes.len() {
                break;
            }

            // Find the opening brace
            let brace_pos = Self::find_char(css_bytes, current_pos, b'{');
            if brace_pos.is_none() {
                break;
            }
            let brace_pos = brace_pos.unwrap();

            // Extract selector
            let selector = String::from_utf8_lossy(&css_bytes[current_pos..brace_pos]).to_string();
            let selector = selector.trim().to_string();

            if selector.is_empty() {
                current_pos = brace_pos + 1;
                continue;
            }

            // Find closing brace
            let close_brace_pos = Self::find_matching_brace(css_bytes, brace_pos + 1);
            if close_brace_pos.is_none() {
                break;
            }
            let close_brace_pos = close_brace_pos.unwrap();

            // Extract declarations
            let declarations_str =
                String::from_utf8_lossy(&css_bytes[brace_pos + 1..close_brace_pos]).to_string();
            let declarations = Self::parse_declarations(&declarations_str);

            if !declarations.is_empty() {
                rules.push(CSSRule {
                    selector,
                    declarations,
                });
            }

            current_pos = close_brace_pos + 1;
        }

        Stylesheet { rules }
    }

    fn skip_whitespace_and_comments(bytes: &[u8], mut pos: usize) -> (usize, bool) {
        while pos < bytes.len() {
            match bytes[pos] {
                b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
                b'/' if pos + 1 < bytes.len() && bytes[pos + 1] == b'*' => {
                    // Skip comment
                    pos += 2;
                    while pos + 1 < bytes.len() {
                        if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                            pos += 2;
                            break;
                        }
                        pos += 1;
                    }
                }
                _ => return (pos, true),
            }
        }
        (pos, false)
    }

    fn find_char(bytes: &[u8], mut pos: usize, target: u8) -> Option<usize> {
        let mut in_string = false;
        let mut string_char = b'\0';

        while pos < bytes.len() {
            let c = bytes[pos];

            if in_string {
                if c == b'\\' && pos + 1 < bytes.len() {
                    pos += 2;
                    continue;
                }
                if c == string_char {
                    in_string = false;
                }
            } else if c == b'"' || c == b'\'' {
                in_string = true;
                string_char = c;
            } else if c == target {
                return Some(pos);
            }

            pos += 1;
        }
        None
    }

    fn find_matching_brace(bytes: &[u8], mut pos: usize) -> Option<usize> {
        let mut depth = 1;
        let mut in_string = false;
        let mut string_char = b'\0';

        while pos < bytes.len() {
            let c = bytes[pos];

            if in_string {
                if c == b'\\' && pos + 1 < bytes.len() {
                    pos += 2;
                    continue;
                }
                if c == string_char {
                    in_string = false;
                }
            } else if c == b'"' || c == b'\'' {
                in_string = true;
                string_char = c;
            } else if c == b'{' {
                depth += 1;
            } else if c == b'}' {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }

            pos += 1;
        }
        None
    }

    fn parse_declarations(decls_str: &str) -> Vec<CSSDeclaration> {
        let mut declarations = Vec::new();

        for property in decls_str.split(';') {
            let property = property.trim();
            if property.is_empty() {
                continue;
            }

            if let Some(colon_pos) = property.find(':') {
                let name = property[..colon_pos].trim().to_string();
                let value = property[colon_pos + 1..].trim().to_string();

                if !name.is_empty() && !value.is_empty() {
                    declarations.push(CSSDeclaration {
                        property: name,
                        value,
                    });
                }
            }
        }

        declarations
    }

    /// Match this stylesheet against a DOM node and return matching declarations
    pub fn match_node(&self, dom: &DomTree, node_id: NodeId) -> Vec<&CSSDeclaration> {
        let mut matched = Vec::new();

        let node = match dom.get_node(node_id) {
            Some(n) => n,
            None => return matched,
        };

        let element = match node.as_element() {
            Some(e) => e,
            None => return matched,
        };

        let tag_name = element.local.as_ref().to_lowercase();
        let id = node.get_attribute("id");
        let classes: Vec<&str> = node
            .get_attribute("class")
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default();

        for rule in &self.rules {
            if Self::selector_matches(&rule.selector, &tag_name, id.as_deref(), &classes) {
                for decl in &rule.declarations {
                    matched.push(decl);
                }
            }
        }

        matched
    }

    fn selector_matches(
        selector: &str,
        tag_name: &str,
        id: Option<&str>,
        classes: &[&str],
    ) -> bool {
        let selector = selector.trim();

        // Universal selector
        if selector == "*" {
            return true;
        }

        // ID selector: #id
        if let Some(id_match) = SELECTOR_ID_REGEX.captures(selector) {
            if let Some(id_value) = id_match.get(1) {
                return id == Some(id_value.as_str());
            }
            return false;
        }

        // Class selector: .class
        if let Some(class_match) = SELECTOR_CLASS_REGEX.captures(selector) {
            if let Some(class_value) = class_match.get(1) {
                return classes.contains(&class_value.as_str());
            }
            return false;
        }

        // Tag selector
        if let Some(tag_match) = SELECTOR_TAG_REGEX.captures(selector) {
            if let Some(tag_value) = tag_match.get(1) {
                return tag_name == tag_value.as_str().to_lowercase();
            }
        }

        // Tag with class: tag.class
        let parts: Vec<&str> = selector.split('.').collect();
        if parts.len() == 2 {
            let selector_tag = parts[0].to_lowercase();
            let selector_class = parts[1];
            return tag_name == selector_tag && classes.contains(&selector_class);
        }

        // Tag with ID: tag#id
        let parts: Vec<&str> = selector.split('#').collect();
        if parts.len() == 2 {
            let selector_tag = parts[0].to_lowercase();
            let selector_id = parts[1];
            return tag_name == selector_tag && id == Some(selector_id);
        }

        // Direct descendant: tag1 tag2 (simplified - just check last part)
        let parts: Vec<&str> = selector.split_whitespace().collect();
        let last_part = parts.last().unwrap_or(&selector);
        if *last_part == tag_name {
            return true;
        }

        false
    }
}

impl ComputedStyles {
    /// Compute styles from an element's inline styles, external stylesheets, and attributes
    pub fn from_element(dom: &DomTree, node_id: NodeId, stylesheets: &[Stylesheet]) -> Self {
        let mut styles = ComputedStyles::default();

        let node = match dom.get_node(node_id) {
            Some(n) => n,
            None => return styles,
        };

        // Get tag name for default styles
        let tag_name = node
            .as_element()
            .map(|q| q.local.as_ref().to_lowercase())
            .unwrap_or_default();

        // Apply tag-specific defaults
        Self::apply_tag_defaults(&tag_name, &mut styles);

        // Apply external stylesheets
        for stylesheet in stylesheets {
            let matched_decls = stylesheet.match_node(dom, node_id);
            for decl in matched_decls {
                Self::apply_declaration(&decl.property, &decl.value, &mut styles);
            }
        }

        // Get inline style attribute (highest priority)
        if let Some(style_attr) = node.get_attribute("style") {
            Self::parse_inline_style(style_attr, &mut styles);
        }

        // Get deprecated bgcolor attribute
        if let Some(bgcolor) = node.get_attribute("bgcolor") {
            if styles.background_color.is_none() {
                styles.background_color = Self::parse_color(&bgcolor);
            }
        }

        // Get deprecated text attribute
        if let Some(text_color) = node.get_attribute("text") {
            styles.color = Self::parse_color(&text_color);
        }

        styles
    }

    /// Legacy constructor for backwards compatibility (no external stylesheets)
    pub fn from_element_legacy(dom: &DomTree, node_id: NodeId) -> Self {
        Self::from_element(dom, node_id, &[])
    }

    fn apply_tag_defaults(tag: &str, styles: &mut ComputedStyles) {
        match tag {
            "body" => {
                styles.background_color = Some([255, 255, 255, 255]); // White background
                styles.color = Some([0, 0, 0, 255]); // Black text
                styles.margin = "8px".to_string();
                styles.padding = "8px".to_string();
            }
            "div" | "p" => {
                styles.margin = "1em 0".to_string();
            }
            "h1" => {
                styles.font_size = "2em".to_string();
                styles.font_family = "sans-serif".to_string();
                styles.margin = "0.67em 0".to_string();
            }
            "h2" => {
                styles.font_size = "1.5em".to_string();
                styles.font_family = "sans-serif".to_string();
                styles.margin = "0.83em 0".to_string();
            }
            "h3" => {
                styles.font_size = "1.17em".to_string();
                styles.font_family = "sans-serif".to_string();
                styles.margin = "1em 0".to_string();
            }
            "h4" | "h5" | "h6" => {
                styles.font_family = "sans-serif".to_string();
                styles.margin = "1.33em 0".to_string();
            }
            "a" => {
                styles.color = Some([0, 0, 238, 255]); // Link blue
            }
            "table" => {
                styles.border = "1px solid black".to_string();
            }
            "td" | "th" => {
                styles.border = "1px solid black".to_string();
                styles.padding = "2px".to_string();
            }
            _ => {}
        }
    }

    fn apply_declaration(property: &str, value: &str, styles: &mut ComputedStyles) {
        match property.to_lowercase().as_str() {
            "width" => styles.width = value.to_string(),
            "height" => styles.height = value.to_string(),
            "padding" => styles.padding = value.to_string(),
            "margin" => styles.margin = value.to_string(),
            "background-color" | "background" => {
                styles.background_color = Self::parse_color(value);
            }
            "color" => {
                styles.color = Self::parse_color(value);
            }
            "font-size" => styles.font_size = value.to_string(),
            "font-family" => styles.font_family = value.to_string(),
            "display" => styles.display = value.to_string(),
            "position" => {
                styles.position = match value.to_lowercase().as_str() {
                    "relative" => Position::Relative,
                    "absolute" => Position::Absolute,
                    "fixed" => Position::Fixed,
                    _ => Position::Static,
                };
            }
            "top" => styles.top = value.to_string(),
            "right" => styles.right = value.to_string(),
            "bottom" => styles.bottom = value.to_string(),
            "left" => styles.left = value.to_string(),
            "z-index" => {
                styles.z_index = value.parse().unwrap_or(0);
            }
            "border" => styles.border = value.to_string(),
            // Flexbox
            "flex-direction" => {
                styles.flex_direction = match value.to_lowercase().as_str() {
                    "row" => FlexDirection::Row,
                    "row-reverse" => FlexDirection::RowReverse,
                    "column" => FlexDirection::Column,
                    "column-reverse" => FlexDirection::ColumnReverse,
                    _ => FlexDirection::default(),
                };
            }
            "flex-wrap" => {
                styles.flex_wrap = match value.to_lowercase().as_str() {
                    "wrap" => FlexWrap::Wrap,
                    "wrap-reverse" => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                };
            }
            "justify-content" => {
                styles.justify_content = match value.to_lowercase().as_str() {
                    "flex-end" => JustifyContent::FlexEnd,
                    "center" => JustifyContent::Center,
                    "space-between" => JustifyContent::SpaceBetween,
                    "space-around" => JustifyContent::SpaceAround,
                    "space-evenly" => JustifyContent::SpaceEvenly,
                    _ => JustifyContent::FlexStart,
                };
            }
            "align-items" => {
                styles.align_items = match value.to_lowercase().as_str() {
                    "flex-start" => AlignItems::FlexStart,
                    "flex-end" => AlignItems::FlexEnd,
                    "center" => AlignItems::Center,
                    "baseline" => AlignItems::Baseline,
                    _ => AlignItems::Stretch,
                };
            }
            "align-self" => {
                styles.align_self = match value.to_lowercase().as_str() {
                    "flex-start" => Some(AlignItems::FlexStart),
                    "flex-end" => Some(AlignItems::FlexEnd),
                    "center" => Some(AlignItems::Center),
                    "baseline" => Some(AlignItems::Baseline),
                    "stretch" => Some(AlignItems::Stretch),
                    _ => None,
                };
            }
            "flex-grow" => {
                styles.flex_grow = value.parse().unwrap_or(0.0);
            }
            "flex-shrink" => {
                styles.flex_shrink = value.parse().unwrap_or(1.0);
            }
            "flex-basis" => {
                styles.flex_basis = value.to_string();
            }
            "gap" | "grid-gap" => {
                styles.gap = value.to_string();
                styles.grid_gap = value.to_string();
            }
            // Grid
            "grid-template-columns" => {
                styles.grid_template_columns = value.to_string();
            }
            "grid-template-rows" => {
                styles.grid_template_rows = value.to_string();
            }
            "box-sizing" => {
                styles.box_sizing = value.to_string();
            }
            _ => {}
        }
    }

    fn parse_inline_style(style_attr: &str, styles: &mut ComputedStyles) {
        // Simple inline style parser - handles basic key: value; pairs
        let style_str = style_attr.trim();
        if style_str.is_empty() {
            return;
        }

        // Split by semicolons to get individual properties
        for property in style_str.split(';') {
            let property = property.trim();
            if property.is_empty() {
                continue;
            }

            // Find the colon separator
            if let Some(colon_pos) = property.find(':') {
                let name = property[..colon_pos].trim().to_string();
                let value = property[colon_pos + 1..].trim().to_string();

                Self::apply_declaration(&name, &value, styles);
            }
        }
    }

    /// Check if this element is a flex container
    pub fn is_flex_container(&self) -> bool {
        self.display.to_lowercase() == "flex"
    }

    /// Check if this element is a grid container
    pub fn is_grid_container(&self) -> bool {
        self.display.to_lowercase() == "grid"
    }

    /// Check if display is none
    pub fn is_display_none(&self) -> bool {
        self.display.to_lowercase() == "none"
    }

    /// Check if element is positioned (relative, absolute, or fixed)
    pub fn is_positioned(&self) -> bool {
        matches!(
            self.position,
            Position::Relative | Position::Absolute | Position::Fixed
        )
    }

    /// Check if element is absolutely positioned
    pub fn is_absolute(&self) -> bool {
        self.position == Position::Absolute
    }

    /// Check if element is fixed positioned
    pub fn is_fixed(&self) -> bool {
        self.position == Position::Fixed
    }

    /// Check if element is relatively positioned
    pub fn is_relative(&self) -> bool {
        self.position == Position::Relative
    }

    /// Parse a position offset value (top, right, bottom, left) against containing size
    /// Returns the offset as f32, or None if "auto"
    pub fn parse_offset(&self, offset: &str, containing_size: f32) -> Option<f32> {
        let offset = offset.trim();
        if offset == "auto" || offset.is_empty() {
            return None;
        }
        if offset.ends_with("px") {
            return offset.trim_end_matches("px").parse().ok();
        }
        if offset.ends_with('%') {
            let pct: f32 = offset.trim_end_matches('%').parse().ok()?;
            return Some(containing_size * pct / 100.0);
        }
        // Try parsing as raw number (px)
        offset.parse().ok()
    }

    /// Get gap value as f32 (returns (row_gap, col_gap))
    pub fn get_gap(&self) -> (f32, f32) {
        let parts: Vec<&str> = self.gap.split_whitespace().collect();
        match parts.len() {
            1 => {
                let v: f32 = parts[0].trim_end_matches("px").parse().unwrap_or(0.0);
                (v, v)
            }
            2 => {
                let row: f32 = parts[0].trim_end_matches("px").parse().unwrap_or(0.0);
                let col: f32 = parts[1].trim_end_matches("px").parse().unwrap_or(0.0);
                (row, col)
            }
            _ => (0.0, 0.0),
        }
    }

    /// Parse grid template columns into a vector of sizes
    pub fn parse_grid_columns(&self) -> Vec<GridTrack> {
        Self::parse_grid_tracks(&self.grid_template_columns)
    }

    /// Parse grid template rows into a vector of sizes
    pub fn parse_grid_rows(&self) -> Vec<GridTrack> {
        Self::parse_grid_tracks(&self.grid_template_rows)
    }

    fn parse_grid_tracks(template: &str) -> Vec<GridTrack> {
        if template.is_empty() {
            return Vec::new();
        }

        let mut tracks = Vec::new();
        for part in template.split_whitespace() {
            let part = part
                .trim_end_matches("px")
                .trim_end_matches("fr")
                .trim_end_matches("%");

            if part == "auto" {
                tracks.push(GridTrack::Auto);
            } else if part.ends_with("fr") {
                let val: f32 = part.trim_end_matches("fr").parse().unwrap_or(1.0);
                tracks.push(GridTrack::Fractional(val));
            } else if part.ends_with('%') {
                let val: f32 = part.trim_end_matches('%').parse().unwrap_or(0.0);
                tracks.push(GridTrack::Percentage(val / 100.0));
            } else if let Some(_px) = part.strip_suffix("px") {
                let val: f32 = _px.parse().unwrap_or(0.0);
                tracks.push(GridTrack::Fixed(val));
            } else if let Ok(val) = part.parse::<f32>() {
                tracks.push(GridTrack::Fixed(val));
            }
        }

        tracks
    }

    fn parse_color(color_str: &str) -> Option<[u8; 4]> {
        let color_str = color_str.trim();

        // Handle named colors
        match color_str.to_lowercase().as_str() {
            "white" => return Some([255, 255, 255, 255]),
            "black" => return Some([0, 0, 0, 255]),
            "red" => return Some([255, 0, 0, 255]),
            "green" => return Some([0, 128, 0, 255]),
            "blue" => return Some([0, 0, 255, 255]),
            "yellow" => return Some([255, 255, 0, 255]),
            "cyan" => return Some([0, 255, 255, 255]),
            "magenta" => return Some([255, 0, 255, 255]),
            "gray" | "grey" => return Some([128, 128, 128, 255]),
            "silver" => return Some([192, 192, 192, 255]),
            "maroon" => return Some([128, 0, 0, 255]),
            "olive" => return Some([128, 128, 0, 255]),
            "navy" => return Some([0, 0, 128, 255]),
            "purple" => return Some([128, 0, 128, 255]),
            "teal" => return Some([0, 128, 128, 255]),
            "orange" => return Some([255, 165, 0, 255]),
            "pink" => return Some([255, 192, 203, 255]),
            "transparent" => return None,
            _ => {}
        }

        // Handle hex colors
        if color_str.starts_with('#') {
            let hex = &color_str[1..];
            match hex.len() {
                3 => {
                    // #RGB format
                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                    return Some([r, g, b, 255]);
                }
                6 => {
                    // #RRGGBB format
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    return Some([r, g, b, 255]);
                }
                8 => {
                    // #RRGGBBAA format
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                    return Some([r, g, b, a]);
                }
                _ => {}
            }
        }

        // Handle rgb() and rgba() formats
        if color_str.starts_with("rgb(") || color_str.starts_with("rgba(") {
            let inner = color_str
                .trim_start_matches("rgb(")
                .trim_start_matches("rgba(")
                .trim_end_matches(')');

            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() >= 3 {
                let r: u8 = parts[0].trim().parse().ok()?;
                let g: u8 = parts[1].trim().parse().ok()?;
                let b: u8 = parts[2].trim().parse().ok()?;
                let a: u8 = parts
                    .get(3)
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(255);
                return Some([r, g, b, a]);
            }
        }

        None
    }
}

/// Grid track representation
#[derive(Debug, Clone)]
pub enum GridTrack {
    /// Fixed size in pixels
    Fixed(f32),
    /// Fractional unit (fr)
    Fractional(f32),
    /// Percentage of container
    Percentage(f32),
    /// Auto-sized
    Auto,
}
