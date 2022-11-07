use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
};

use petgraph::Graph;

pub fn get_depths<T>(edges: &Vec<(T, T)>) -> Vec<(T, Vec<(T, Vec<T>)>)>
where
    T: Clone + Hash + Eq + Debug,
{
    // find all roots and execute a bfs from each one to get depths
    // of each node from each root
    let targets = edges.iter().map(|e| e.1.clone()).collect::<HashSet<_>>();
    let mut roots = HashSet::new();
    for (s, _) in edges {
        if !targets.contains(&s) {
            roots.insert(s);
        }
    }

    roots
        .iter()
        .map(|&r| (r.clone(), get_root_paths(r, edges)))
        .collect()
}

fn get_root_paths<T>(root: &T, edges: &Vec<(T, T)>) -> Vec<(T, Vec<T>)>
where
    T: Clone + Hash + Eq + Debug,
{
    // build graph
    let mut to_graph_node = HashMap::new();
    let mut graph: Graph<T, ()> = Graph::new();

    for (s, t) in edges {
        if !to_graph_node.contains_key(s) {
            to_graph_node.insert(s, graph.add_node(s.clone()));
        }

        if !to_graph_node.contains_key(t) {
            to_graph_node.insert(t, graph.add_node(t.clone()));
        }

        graph.add_edge(
            *to_graph_node.get(s).unwrap(),
            *to_graph_node.get(t).unwrap(),
            (),
        );
    }

    // run bfs
    let mut graph_depths = vec![];

    let mut paths = vec![vec![*to_graph_node.get(root).unwrap()]];
    let mut visited = HashSet::new();
    while !paths.is_empty() {
        let mut new_paths = vec![];
        for path in paths {
            let path_head = *path.last().unwrap();
            if !visited.contains(&path_head) {
                graph_depths.push((path_head.clone(), path.clone()));
                visited.insert(path_head);

                for neighbor in graph.neighbors(path_head) {
                    let mut new_path = path.clone();
                    new_path.push(neighbor);
                    new_paths.push(new_path);
                }
            }
        }

        paths = new_paths;
    }

    // convert graph nodes to real nodes
    graph_depths
        .iter()
        .map(|(n, d)| {
            (
                graph.node_weight(*n).unwrap().clone(),
                d.iter()
                    .map(|p| graph.node_weight(*p).unwrap().clone())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use crate::get_depths;

    #[test]
    fn test_get_depths() {
        assert_eq!(
            get_depths(&(vec![(0, 1), (1, 2), (2, 3)])),
            vec![(
                0,
                vec![
                    (0, vec![0]),
                    (1, vec![0, 1]),
                    (2, vec![0, 1, 2]),
                    (3, vec![0, 1, 2, 3]),
                ]
            )]
        );
    }

    #[test]
    fn test_get_depths_2_roots() {
        let depths = get_depths(&vec![
            (0, 1), // root 1
            (1, 2),
            (2, 3),
            (10, 11), // root 2
            (11, 12),
            (12, 13),
        ]);

        assert!(depths.contains(&(
            0,
            vec![
                (0, vec![0]),
                (1, vec![0, 1]),
                (2, vec![0, 1, 2]),
                (3, vec![0, 1, 2, 3]),
            ]
        )));
        assert!(depths.contains(&(
            10,
            vec![
                (10, vec![10]),
                (11, vec![10, 11]),
                (12, vec![10, 11, 12]),
                (13, vec![10, 11, 12, 13]),
            ]
        )));
    }

    #[test]
    fn test_get_depths_loop() {
        assert_eq!(
            get_depths(&(vec![(0, 1), (0, 2), (1, 2), (2, 1)])),
            vec![(0, vec![(0, vec![0]), (2, vec![0, 2]), (1, vec![0, 1]),])]
        );
    }

    #[test]
    fn test_top_level_loop() {
        assert_eq!(get_depths(&(vec![(0, 1), (1, 0)])), vec![]);
    }
}
