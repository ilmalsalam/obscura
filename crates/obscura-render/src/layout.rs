//! Layout module - converts DOM tree to laid-out boxes

use crate::style::{
    AlignItems, ComputedStyles, FlexDirection, GridTrack, JustifyContent, Position, Stylesheet,
};
use crate::RenderError;
use obscura_dom::tree::{DomTree, NodeId};

/// A laid-out box with position and dimensions
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// X position from left
    pub x: f32,
    /// Y position from top
    pub y: f32,
    /// Content width
    pub width: f32,
    /// Content height
    pub height: f32,
    /// Box model: padding, border, margin
    pub padding: BoxEdges,
    pub border: BoxEdges,
    pub margin: BoxEdges,
    /// Background color (RGBA)
    pub background_color: Option<[u8; 4]>,
    /// Text content if this is a text node
    pub text_content: Option<String>,
    /// Children boxes
    pub children: Vec<LayoutBox>,
    /// Node ID in DOM tree (for debugging)
    pub node_id: Option<NodeId>,
    /// Z-index for stacking context
    pub z_index: i32,
    /// Position mode
    pub position: Position,
    /// Position offsets (top, right, bottom, left)
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    /// Whether this box establishes a containing block for absolutely positioned children
    pub is_containing_block: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BoxEdges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BoxEdges {
    pub fn new(all: f32) -> Self {
        BoxEdges {
            top: all,
            right: all,
            bottom: all,
            left: all,
        }
    }
}

/// The complete layout tree
#[derive(Debug)]
pub struct LayoutTree {
    pub root: Option<LayoutBox>,
    pub width: u32,
    pub height: u32,
}

impl LayoutTree {
    /// Build a layout tree from a DOM tree (legacy version without stylesheets)
    pub fn build(dom: &DomTree, width: u32, height: u32) -> Self {
        Self::build_with_stylesheets(dom, width, height, &[])
    }

    /// Build a layout tree from a DOM tree with external stylesheets
    pub fn build_with_stylesheets(
        dom: &DomTree,
        width: u32,
        height: u32,
        stylesheets: &[Stylesheet],
    ) -> Self {
        let body_id = dom.find_body_or_root();
        let root = Self::build_box(
            dom,
            body_id,
            0.0,
            0.0,
            width as f32,
            height as f32,
            stylesheets,
        );

        LayoutTree {
            root,
            width,
            height,
        }
    }

    fn build_box(
        dom: &DomTree,
        node_id: NodeId,
        x: f32,
        y: f32,
        containing_width: f32,
        containing_height: f32,
        stylesheets: &[Stylesheet],
    ) -> Option<LayoutBox> {
        let node = dom.get_node(node_id)?;

        // Handle text nodes
        if node.is_text() {
            let text = dom.text_content(node_id);
            if text.trim().is_empty() {
                return None;
            }
            return Some(LayoutBox {
                x,
                y,
                width: containing_width,
                height: 20.0, // Approximate text height
                padding: BoxEdges::default(),
                border: BoxEdges::default(),
                margin: BoxEdges::default(),
                background_color: None,
                text_content: Some(text),
                children: vec![],
                node_id: Some(node_id),
                z_index: 0,
                position: Position::Static,
                top: None,
                right: None,
                bottom: None,
                left: None,
                is_containing_block: false,
            });
        }

        if !node.is_element() {
            return None;
        }

        // Get computed styles (now with stylesheets)
        let styles = ComputedStyles::from_element(dom, node_id, stylesheets);

        // Check for display: none
        if styles.is_display_none() {
            return None;
        }

        // Calculate dimensions based on element type and styles
        let tag_name = node
            .as_element()
            .map(|q| q.local.as_ref().to_string())
            .unwrap_or_default();

        let (width, mut height) =
            Self::calculate_element_size(&tag_name, &styles, containing_width);
        let padding = Self::parse_padding(&styles.padding, width);
        let margin = Self::parse_margin(&styles.margin, width);

        // Apply margin
        let current_x = x + margin.left;
        let current_y = y + margin.top;

        // Build children first so we can use their sizes for flex/grid layout
        let mut children = Vec::new();
        let mut child_y = current_y + padding.top;

        for child_id in dom.children(node_id) {
            let child_width = width - padding.left - padding.right - margin.left - margin.right;
            if let Some(child_box) = Self::build_box(
                dom,
                child_id,
                current_x + padding.left,
                child_y,
                child_width.max(1.0),
                height.max(1.0), // Pass current height as containing height for children
                stylesheets,
            ) {
                child_y += child_box.margin.top + child_box.height + child_box.margin.bottom;
                children.push(child_box);
            }
        }

        // Layout children based on display type
        if styles.is_flex_container() {
            Self::layout_flex_children(&mut children, &styles, width);
        } else if styles.is_grid_container() {
            Self::layout_grid_children(&mut children, &styles, width, &padding, &margin);
        } else {
            // Block layout (default)
            Self::layout_block_children(&mut children, current_x, current_y, &padding);
        }

        // Apply positioning to children (relative, absolute, fixed)
        Self::layout_positioned_children(
            &mut children,
            &styles,
            current_x,
            current_y,
            width,
            height,
        );

        // Sort children by z-index (painter's algorithm - lower z-index first)
        Self::sort_children_by_z_index(&mut children);

        // Update our height to fit children if needed
        if height <= 0.0 {
            height = Self::calculate_content_height(&children, &styles, &padding);
        }
        if height <= 0.0 {
            height = 20.0; // Minimum height
        }

        // Determine if this box establishes a containing block for absolutely positioned children
        // A containing block is established by:
        // - position: relative|absolute|fixed
        // - display: flex|grid
        // - block container boxes (block, inline-block, flow-root)
        let is_containing_block = styles.is_positioned()
            || styles.is_flex_container()
            || styles.is_grid_container()
            || (styles.display.is_empty() || styles.display == "block");

        Some(LayoutBox {
            x: current_x,
            y: current_y,
            width,
            height,
            padding,
            border: BoxEdges::default(),
            margin,
            background_color: styles.background_color,
            text_content: None,
            children,
            node_id: Some(node_id),
            z_index: styles.z_index,
            position: styles.position,
            top: styles.parse_offset(&styles.top, containing_height),
            right: styles.parse_offset(&styles.right, containing_width),
            bottom: styles.parse_offset(&styles.bottom, containing_height),
            left: styles.parse_offset(&styles.left, containing_width),
            is_containing_block,
        })
    }

    fn layout_block_children(children: &mut Vec<LayoutBox>, x: f32, y: f32, padding: &BoxEdges) {
        let mut current_y = y + padding.top;
        for child in children.iter_mut() {
            child.x = x + padding.left;
            child.y = current_y + child.margin.top;
            current_y = child.y + child.height + child.margin.bottom;
        }
    }

    /// Apply positioning offsets to children based on their position property
    /// - Relative: offset from normal position
    /// - Absolute: positioned relative to containing block
    /// - Fixed: positioned relative to viewport
    fn layout_positioned_children(
        children: &mut Vec<LayoutBox>,
        _parent_styles: &ComputedStyles,
        parent_x: f32,
        parent_y: f32,
        parent_width: f32,
        parent_height: f32,
    ) {
        let parent_padding_box_x = parent_x;
        let parent_padding_box_y = parent_y;
        let parent_padding_box_width = parent_width;
        let parent_padding_box_height = parent_height;

        for child in children.iter_mut() {
            match child.position {
                Position::Relative => {
                    // Apply relative positioning as offset from normal position
                    if let Some(top) = child.top {
                        child.y += top;
                    }
                    if let Some(left) = child.left {
                        child.x += left;
                    }
                    // Right and bottom are handled by adjusting width/height if needed
                    // For now, we focus on top/left as that's most common
                }
                Position::Absolute => {
                    // Find containing block for absolute positioning
                    // The containing block is the nearest positioned ancestor
                    // For now, we use the parent as the containing block
                    // since positioned children are typically within positioned parents

                    // Position relative to the containing block (padding box)
                    let containing_x = parent_padding_box_x;
                    let containing_y = parent_padding_box_y;
                    let containing_w = parent_padding_box_width;
                    let containing_h = parent_padding_box_height;

                    // Apply position offsets
                    // If none specified, use "auto" behavior (keep normal position)
                    if let Some(top) = child.top {
                        child.y = containing_y + top;
                    } else if let Some(bottom) = child.bottom {
                        child.y = containing_y + containing_h - bottom - child.height;
                    }
                    // else: use normal flow position

                    if let Some(left) = child.left {
                        child.x = containing_x + left;
                    } else if let Some(right) = child.right {
                        child.x = containing_x + containing_w - right - child.width;
                    }
                    // else: use normal flow position
                }
                Position::Fixed => {
                    // Fixed positioning is relative to the viewport
                    // The viewport is the root of the layout tree
                    // For now, we position at the given offset from (0, 0)
                    if let Some(top) = child.top {
                        child.y = top;
                    }
                    if let Some(left) = child.left {
                        child.x = left;
                    }
                    // Right and bottom offsets
                    if let Some(bottom) = child.bottom {
                        // Not typically used with fixed, but handle it
                        child.y = bottom;
                    }
                    if let Some(right) = child.right {
                        child.x = right;
                    }
                }
                Position::Static => {
                    // Static positioning - no adjustment needed
                }
            }
        }
    }

    /// Sort children by z-index for painter's algorithm rendering
    fn sort_children_by_z_index(children: &mut Vec<LayoutBox>) {
        // Stable sort to preserve DOM order for equal z-index values
        let mut sorted_indices: Vec<usize> = (0..children.len()).collect();
        sorted_indices.sort_by(|&a, &b| children[a].z_index.cmp(&children[b].z_index));

        // Reorder children based on sorted indices
        let original_children = children.clone();
        for (new_idx, &old_idx) in sorted_indices.iter().enumerate() {
            children[new_idx] = original_children[old_idx].clone();
        }
    }

    fn layout_flex_children(
        children: &mut Vec<LayoutBox>,
        styles: &ComputedStyles,
        container_width: f32,
    ) {
        if children.is_empty() {
            return;
        }

        let (row_gap, col_gap) = styles.get_gap();
        let is_row = matches!(
            styles.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        let total_gap = if is_row { col_gap } else { row_gap };
        let num_children = children.len();

        // Calculate total children size
        let total_children_size: f32 = children
            .iter()
            .map(|c| if is_row { c.width } else { c.height })
            .sum();
        let total_gaps = total_gap * (num_children - 1) as f32;
        let available_space = container_width - total_children_size - total_gaps;
        let initial_offset = match styles.justify_content {
            JustifyContent::FlexStart => 0.0,
            JustifyContent::FlexEnd => available_space.max(0.0),
            JustifyContent::Center => (available_space / 2.0).max(0.0),
            JustifyContent::SpaceBetween => 0.0,
            JustifyContent::SpaceAround => (available_space / (num_children * 2) as f32).max(0.0),
            JustifyContent::SpaceEvenly => (available_space / (num_children + 1) as f32).max(0.0),
        };

        let mut current_pos: f32 = initial_offset;

        // Calculate indices based on direction
        let indices: Vec<usize> = if matches!(
            styles.flex_direction,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        ) {
            (0..num_children).rev().collect()
        } else {
            (0..num_children).collect()
        };

        // Space between spacing requires pre-calculation
        let space_between_spacing =
            if num_children > 1 && matches!(styles.justify_content, JustifyContent::SpaceBetween) {
                available_space / (num_children - 1) as f32
            } else {
                0.0
            };

        for (i, &idx) in indices.iter().enumerate() {
            let child = &mut children[idx];

            if is_row {
                // Horizontal layout
                child.x = current_pos;
                child.y = match styles.align_items {
                    AlignItems::FlexStart | AlignItems::Stretch => 0.0,
                    AlignItems::FlexEnd => 0.0, // Would need container height
                    AlignItems::Center => 0.0,
                    AlignItems::Baseline => 0.0,
                };

                // Update position for next child
                if matches!(styles.justify_content, JustifyContent::SpaceBetween) {
                    current_pos += child.width + space_between_spacing;
                } else {
                    current_pos += child.width + total_gap;
                }
            } else {
                // Vertical layout
                child.x = match styles.align_items {
                    AlignItems::FlexStart | AlignItems::Stretch => 0.0,
                    AlignItems::FlexEnd | AlignItems::Center => 0.0,
                    _ => 0.0,
                };
                child.y = current_pos;

                // Update position for next child
                if matches!(styles.justify_content, JustifyContent::SpaceBetween) {
                    current_pos += child.height + space_between_spacing;
                } else {
                    current_pos += child.height + total_gap;
                }
            }
        }
    }

    fn layout_grid_children(
        children: &mut Vec<LayoutBox>,
        styles: &ComputedStyles,
        container_width: f32,
        padding: &BoxEdges,
        margin: &BoxEdges,
    ) {
        if children.is_empty() {
            return;
        }

        let columns = styles.parse_grid_columns();
        if columns.is_empty() {
            // Fallback: single column layout
            Self::layout_block_children(children, margin.left, margin.top, padding);
            return;
        }

        let (row_gap, col_gap) = styles.get_gap();
        let inner_width =
            container_width - padding.left - padding.right - margin.left - margin.right;

        // Calculate column widths
        let mut column_widths: Vec<f32> = Vec::new();
        let mut total_fr = 0.0;
        let mut total_fixed = 0.0;
        let mut num_auto = 0;

        for track in &columns {
            match track {
                GridTrack::Fixed(w) => total_fixed += w,
                GridTrack::Fractional(fr) => total_fr += fr,
                GridTrack::Percentage(p) => {
                    column_widths.push(inner_width * p);
                }
                GridTrack::Auto => num_auto += 1,
            }
        }

        let remaining =
            (inner_width - total_fixed - (col_gap * (columns.len() - 1) as f32)).max(0.0);

        for (i, track) in columns.iter().enumerate() {
            let width = match track {
                GridTrack::Fixed(w) => *w,
                GridTrack::Fractional(fr) => {
                    if total_fr > 0.0 {
                        (remaining * fr / total_fr).max(0.0)
                    } else {
                        remaining / num_auto as f32
                    }
                }
                GridTrack::Percentage(_) => column_widths[i], // Already calculated
                GridTrack::Auto => {
                    if num_auto > 0 {
                        remaining / num_auto as f32
                    } else {
                        0.0
                    }
                }
            };
            column_widths.push(width);
        }

        // Position children in grid
        let num_cols = columns.len();
        for (i, child) in children.iter_mut().enumerate() {
            let col = i % num_cols;
            let row = i / num_cols;

            let x_offset = column_widths[..col].iter().sum::<f32>() + (col as f32 * col_gap);
            let y_offset = row as f32 * (child.height + row_gap);

            child.x = margin.left + padding.left + x_offset;
            child.y = margin.top + padding.top + y_offset;
        }
    }

    fn calculate_content_height(
        children: &[LayoutBox],
        styles: &ComputedStyles,
        padding: &BoxEdges,
    ) -> f32 {
        if children.is_empty() {
            return padding.top + padding.bottom;
        }

        if styles.is_flex_container() {
            if matches!(
                styles.flex_direction,
                FlexDirection::Column | FlexDirection::ColumnReverse
            ) {
                children
                    .iter()
                    .map(|c| c.height + c.margin.top + c.margin.bottom)
                    .sum::<f32>()
                    + padding.top
                    + padding.bottom
            } else {
                // In row flexbox, height is determined by the tallest child
                children
                    .iter()
                    .map(|c| c.height)
                    .fold(0.0f32, |a, b| a.max(b))
                    + padding.top
                    + padding.bottom
            }
        } else if styles.is_grid_container() {
            // Calculate based on rows
            let num_cols = styles.parse_grid_columns().len().max(1);
            let num_rows = (children.len() + num_cols - 1) / num_cols;
            let (row_gap, _) = styles.get_gap();

            let mut row_heights: Vec<f32> = vec![0.0; num_rows];
            for (i, child) in children.iter().enumerate() {
                let row = i / num_cols;
                row_heights[row] = row_heights[row].max(child.height);
            }

            row_heights.iter().sum::<f32>()
                + ((num_rows as f32 - 1.0) * row_gap)
                + padding.top
                + padding.bottom
        } else {
            // Block layout: find the bottom of the last child
            let first_child_top = children.first().map(|c| c.y - c.margin.top).unwrap_or(0.0);
            let last_child_bottom = children
                .last()
                .map(|c| c.y + c.height + c.margin.bottom)
                .unwrap_or(0.0);

            last_child_bottom - first_child_top + padding.top + padding.bottom
        }
    }

    fn calculate_element_size(
        _tag: &str,
        styles: &ComputedStyles,
        containing_width: f32,
    ) -> (f32, f32) {
        let width = match styles.width.as_str() {
            "auto" => containing_width,
            s if s.ends_with("px") => s.trim_end_matches("px").parse().unwrap_or(containing_width),
            s if s.ends_with('%') => {
                let pct: f32 = s.trim_end_matches('%').parse().unwrap_or(100.0);
                containing_width * pct / 100.0
            }
            _ => containing_width,
        };

        let height = match styles.height.as_str() {
            "auto" => 0.0,
            s if s.ends_with("px") => s.trim_end_matches("px").parse().unwrap_or(0.0),
            s if s.ends_with('%') => {
                let pct: f32 = s.trim_end_matches('%').parse().unwrap_or(100.0);
                containing_width * pct / 100.0 // Use containing width as base for percentage height
            }
            _ => 0.0,
        };

        (width.max(1.0), height)
    }

    fn parse_padding(padding_str: &str, _width: f32) -> BoxEdges {
        let parts: Vec<&str> = padding_str.split_whitespace().collect();
        match parts.len() {
            1 => {
                let v: f32 = parts[0].trim_end_matches("px").parse().unwrap_or(0.0);
                BoxEdges::new(v)
            }
            2 => {
                let v: f32 = parts[0].trim_end_matches("px").parse().unwrap_or(0.0);
                let h: f32 = parts[1].trim_end_matches("px").parse().unwrap_or(0.0);
                BoxEdges {
                    top: v,
                    right: h,
                    bottom: v,
                    left: h,
                }
            }
            4 => BoxEdges {
                top: parts[0].trim_end_matches("px").parse().unwrap_or(0.0),
                right: parts[1].trim_end_matches("px").parse().unwrap_or(0.0),
                bottom: parts[2].trim_end_matches("px").parse().unwrap_or(0.0),
                left: parts[3].trim_end_matches("px").parse().unwrap_or(0.0),
            },
            _ => BoxEdges::default(),
        }
    }

    fn parse_margin(margin_str: &str, _width: f32) -> BoxEdges {
        let parts: Vec<&str> = margin_str.split_whitespace().collect();
        match parts.len() {
            1 => {
                let v: f32 = parts[0].trim_end_matches("px").parse().unwrap_or(0.0);
                BoxEdges::new(v)
            }
            2 => {
                let v: f32 = parts[0].trim_end_matches("px").parse().unwrap_or(0.0);
                let h: f32 = parts[1].trim_end_matches("px").parse().unwrap_or(0.0);
                BoxEdges {
                    top: v,
                    right: h,
                    bottom: v,
                    left: h,
                }
            }
            4 => BoxEdges {
                top: parts[0].trim_end_matches("px").parse().unwrap_or(0.0),
                right: parts[1].trim_end_matches("px").parse().unwrap_or(0.0),
                bottom: parts[2].trim_end_matches("px").parse().unwrap_or(0.0),
                left: parts[3].trim_end_matches("px").parse().unwrap_or(0.0),
            },
            _ => BoxEdges::default(),
        }
    }

    fn is_block_element(tag: &str) -> bool {
        let lower = tag.to_lowercase();
        matches!(
            lower.as_str(),
            "div"
                | "p"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "ul"
                | "ol"
                | "li"
                | "table"
                | "tr"
                | "td"
                | "th"
                | "section"
                | "article"
                | "header"
                | "footer"
                | "nav"
                | "main"
                | "aside"
                | "blockquote"
                | "hr"
                | "form"
        )
    }
}

/// Render context passed during rendering
pub struct RenderContext<'a> {
    pub pixmap: &'a mut tiny_skia::Pixmap,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub font_system: cosmic_text::FontSystem,
    pub swash_cache: cosmic_text::SwashCache,
}

impl LayoutTree {
    /// Render the layout tree to the pixel buffer
    pub fn render(&self, ctx: &mut RenderContext) -> Result<(), RenderError> {
        if let Some(ref root) = self.root {
            Self::render_box(root, ctx)?;
        }
        Ok(())
    }

    fn render_box(lbox: &LayoutBox, ctx: &mut RenderContext) -> Result<(), RenderError> {
        let screen_x = (lbox.x * ctx.scale) as i32;
        let screen_y = (lbox.y * ctx.scale) as i32;
        let screen_w = (lbox.width * ctx.scale) as u32;
        let screen_h = (lbox.height * ctx.scale) as u32;

        // Don't render if completely off screen
        if screen_x >= ctx.width as i32 || screen_y >= ctx.height as i32 {
            return Ok(());
        }

        // Don't render if negative dimensions
        if screen_w == 0 || screen_h == 0 {
            return Ok(());
        }

        // Draw background
        if let Some(bg) = lbox.background_color {
            let color = tiny_skia::Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]);
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(color);

            let rect = tiny_skia::Rect::from_xywh(
                screen_x as f32,
                screen_y as f32,
                screen_w as f32,
                screen_h as f32,
            );
            if let Some(rect) = rect {
                ctx.pixmap
                    .fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            }
        }

        // Draw border
        if lbox.border.top > 0.0
            || lbox.border.right > 0.0
            || lbox.border.bottom > 0.0
            || lbox.border.left > 0.0
        {
            let border_color = tiny_skia::Color::from_rgba8(100, 100, 100, 255);
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(border_color);

            // Top border
            if lbox.border.top > 0.0 {
                let rect = tiny_skia::Rect::from_xywh(
                    screen_x as f32,
                    screen_y as f32,
                    screen_w as f32,
                    lbox.border.top,
                );
                if let Some(rect) = rect {
                    ctx.pixmap
                        .fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                }
            }
        }

        // Draw text content
        if let Some(ref text) = lbox.text_content {
            if !text.trim().is_empty() {
                let bg = lbox.background_color.unwrap_or([255, 255, 255, 255]);
                Self::render_text(text, screen_x, screen_y, screen_w, bg, ctx)?;
            }
        }

        // Render children
        for child in &lbox.children {
            Self::render_box(child, ctx)?;
        }

        Ok(())
    }

    fn render_text(
        text: &str,
        x: i32,
        y: i32,
        max_width: u32,
        bg_color: [u8; 4],
        ctx: &mut RenderContext,
    ) -> Result<(), RenderError> {
        // Choose text color based on background
        let text_color = if (bg_color[0] as u32 + bg_color[1] as u32 + bg_color[2] as u32) > 383 {
            // Dark text on light bg
            cosmic_text::Color::rgb(0, 0, 0)
        } else {
            // Light text on dark bg
            cosmic_text::Color::rgb(255, 255, 255)
        };

        Self::draw_text_simple(text, x, y, max_width, text_color, ctx)?;

        Ok(())
    }

    fn draw_text_simple(
        text: &str,
        x: i32,
        y: i32,
        max_width: u32,
        color: cosmic_text::Color,
        ctx: &mut RenderContext,
    ) -> Result<(), RenderError> {
        use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

        let metrics = Metrics::new(14.0, 20.0);
        let mut buffer = Buffer::new(&mut ctx.font_system, metrics);

        {
            let mut buffer = buffer.borrow_with(&mut ctx.font_system);
            buffer.set_size(max_width as f32, 1000.0);
            let attrs = Attrs::new();
            buffer.set_text(text, attrs, Shaping::Advanced);
            buffer.shape_until_scroll(true);

            // Draw the text using the swash cache
            buffer.draw(&mut ctx.swash_cache, color, |px, py, w, h, c| {
                if w == 0 || h == 0 {
                    return;
                }
                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(c.r(), c.g(), c.b(), c.a());
                paint.anti_alias = true;

                let rect = tiny_skia::Rect::from_xywh(
                    x as f32 + px as f32,
                    y as f32 + py as f32,
                    w as f32,
                    h as f32,
                );
                if let Some(rect) = rect {
                    ctx.pixmap
                        .fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                }
            });
        }

        Ok(())
    }
}
