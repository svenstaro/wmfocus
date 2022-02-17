use anyhow::{Context, Result};
use log::{debug, info};
use swayipc::{Connection, Node, NodeLayout, NodeType, Workspace};

use crate::DesktopWindow;

/// Find first `Node` that fulfills a given criterion.
fn find_first_node_with_attr<F>(start_node: &Node, predicate: F) -> Option<&Node>
where
    F: Fn(&Node) -> bool,
{
    let mut nodes_to_explore: Vec<&Node> = start_node.nodes.iter().collect();
    while !nodes_to_explore.is_empty() {
        let mut next_vec = vec![];
        for node in &nodes_to_explore {
            if predicate(node) {
                return Some(node);
            }
            next_vec.extend(node.nodes.iter());
        }
        nodes_to_explore = next_vec;
    }
    None
}

/// Find parent of `child`.
fn find_parent_of<'a>(start_node: &'a Node, child: &'a Node) -> Option<&'a Node> {
    let mut nodes_to_explore: Vec<&Node> = start_node.nodes.iter().collect();
    while !nodes_to_explore.is_empty() {
        let mut next_vec = vec![];
        for node in &nodes_to_explore {
            if node.nodes.iter().any(|x| child.id == x.id) {
                return Some(node);
            }
            next_vec.extend(node.nodes.iter());
        }
        nodes_to_explore = next_vec;
    }
    None
}

/// Return a list of all `DesktopWindow`s for the given `Workspace`.
fn crawl_windows(root_node: &Node, workspace: &Workspace) -> Result<Vec<DesktopWindow>> {
    let workspace_node = find_first_node_with_attr(root_node, |x| {
        x.name == Some(workspace.name.clone()) && x.node_type == NodeType::Workspace
    })
    .context("Couldn't find the Workspace node")?;

    let mut nodes_to_explore: Vec<&Node> = workspace_node.nodes.iter().collect();
    nodes_to_explore.extend(workspace_node.floating_nodes.iter());
    let mut windows = vec![];
    while !nodes_to_explore.is_empty() {
        let mut next_vec = vec![];
        for node in &nodes_to_explore {
            next_vec.extend(node.nodes.iter());
            next_vec.extend(node.floating_nodes.iter());

            let root_node = find_parent_of(root_node, node);

            let (pos_x, size_x) = if let Some(root_node) = root_node {
                if root_node.layout == NodeLayout::Tabbed {
                    (node.rect.x + node.deco_rect.x, node.deco_rect.width)
                } else {
                    (node.rect.x, node.rect.width)
                }
            } else {
                (node.rect.x, node.rect.width)
            };

            let pos_y = if let Some(root_node) = root_node {
                if root_node.layout == NodeLayout::Stacked {
                    root_node.rect.y + node.deco_rect.y
                } else {
                    node.rect.y - node.deco_rect.height
                }
            } else {
                node.rect.y - node.deco_rect.height
            };

            let window = DesktopWindow {
                id: node.id,
                x_window_id: node.window,
                pos: (pos_x, pos_y),
                size: (size_x, (node.rect.height + node.deco_rect.height)),
                is_focused: node.focused,
            };
            debug!("Found {:?}", window);
            windows.push(window);
        }
        nodes_to_explore = next_vec;
    }
    Ok(windows)
}

/// Return a list of all windows.
pub fn get_windows() -> Result<Vec<DesktopWindow>> {
    // Establish a connection to Sway over a Unix socket
    let mut connection = Connection::new().expect("Couldn't acquire Sway IPC connection");
    let workspaces = connection
        .get_workspaces()
        .expect("Problem communicating with IPC");
    let visible_workspaces = workspaces.iter().filter(|w| w.visible);
    let root_node = connection.get_tree()?;
    let mut windows = vec![];
    for workspace in visible_workspaces {
        windows.extend(crawl_windows(&root_node, workspace)?);
    }
    Ok(windows)
}

/// Focus a specific `window`.
pub fn focus_window(window: &DesktopWindow) -> Result<()> {
    let mut connection = Connection::new().expect("Couldn't acquire Sway IPC connection");
    let command_str = format!("[con_id=\"{}\"] focus", window.id);
    let command = connection
        .run_command(&command_str)
        .context("Couldn't communicate with Sway")?;
    info!("Sending to Sway: {:?}", command);
    Ok(())
}
