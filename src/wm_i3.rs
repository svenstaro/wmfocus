use i3ipc::reply::{Node, NodeType, Workspace};
use i3ipc::{I3Connection, MessageError};

#[derive(Debug)]
struct Window {
    id: i64,
    rect: (i32, i32, i32, i32),
}

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

/// Return a list of all `Window`s for the given `Workspace`.
fn crawl_windows(root_node: &Node, workspace: &Workspace) -> Vec<Window> {
    let workspace_node = find_first_node_with_attr(&root_node, |x| {
        x.name == Some(workspace.name.clone()) && if let NodeType::Workspace = x.nodetype {
            true
        } else {
            false
        }
    }).expect("Couldn't find the Workspace node");

    let mut nodes_to_explore: Vec<&Node> = workspace_node.nodes.iter().collect();
    nodes_to_explore.extend(workspace_node.floating_nodes.iter());
    let mut windows = vec![];
    while !nodes_to_explore.is_empty() {
        let mut next_vec = vec![];
        for node in &nodes_to_explore {
            next_vec.extend(node.nodes.iter());
            next_vec.extend(node.floating_nodes.iter());
            if node.window.is_some() {
                let window = Window {
                    id: node.id,
                    rect: node.rect,
                };
                windows.push(window);
            }
        }
        nodes_to_explore = next_vec;
    }
    windows
}

pub fn thing() {
    // establish a connection to i3 over a unix socket
    let mut connection = I3Connection::connect().unwrap();
    let workspaces = connection
        .get_workspaces()
        .expect("Problem communicating with i3")
        .workspaces;
    let visible_workspaces = workspaces
        .iter()
        .filter(|w| w.visible);
    let root_node = connection.get_tree().expect("Uh");
    let mut windows = vec![];
    for workspace in visible_workspaces {
        windows = crawl_windows(&root_node, &workspace);
    }
    println!("{:#?}", windows);

    // request and print the i3 version
    // println!("{:?}", connection.get_tree().unwrap());

    // fullscreen the focused window
    // connection.run_command("fullscreen").unwrap();
}
