use anyhow::{Context, Result};
use i3ipc::reply::{Node, NodeLayout, NodeType, Workspace};
use i3ipc::I3Connection;
use log::{debug, info};

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
        x.name == Some(workspace.name.clone()) && x.nodetype == NodeType::Workspace
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
            if node.window.is_some() {
                let root_node = find_parent_of(root_node, node);

                let (pos_x, size_x) = if let Some(root_node) = root_node {
                    if root_node.layout == NodeLayout::Tabbed {
                        (node.rect.0 + node.deco_rect.0, node.deco_rect.2)
                    } else {
                        (node.rect.0, node.rect.2)
                    }
                } else {
                    (node.rect.0, node.rect.2)
                };

                let pos_y = if let Some(root_node) = root_node {
                    if root_node.layout == NodeLayout::Stacked {
                        root_node.rect.1 + node.deco_rect.1
                    } else {
                        node.rect.1 + node.deco_rect.3
                    }
                } else {
                    node.rect.1 + node.deco_rect.3
                };

                let window = DesktopWindow {
                    id: node.id,
                    x_window_id: node.window,
                    pos: (pos_x, pos_y),
                    size: (size_x, (node.rect.3 + node.deco_rect.3)),
                    is_focused: node.focused,
                };
                debug!("Found {:?}", window);
                windows.push(window);
            }
        }
        nodes_to_explore = next_vec;
    }
    Ok(windows)
}

/// Return a list of all windows.
pub fn get_windows() -> Result<Vec<DesktopWindow>> {
    // Establish a connection to i3 over a unix socket
    let mut connection = I3Connection::connect().context("Couldn't acquire i3 connection")?;
    let workspaces = connection
        .get_workspaces()
        .context("Problem communicating with i3")?
        .workspaces;
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
    let mut connection = I3Connection::connect().context("Couldn't acquire i3 connection")?;
    let command_str = format!("[con_id=\"{}\"] focus", window.id);
    let command = connection
        .run_command(&command_str)
        .context("Couldn't communicate with i3")?;
    info!("Sending to i3: {:?}", command);
    Ok(())
}
