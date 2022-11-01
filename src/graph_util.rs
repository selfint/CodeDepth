use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use petgraph::Graph;

pub fn get_depths<T>(edges: &Vec<(T, T)>) -> Vec<(T, Vec<(T, usize)>)>
where
    T: Clone + Hash + Eq + std::fmt::Debug,
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
        .map(|&r| (r.clone(), get_root_depths(r, edges)))
        .collect()
}

fn get_root_depths<T>(root: &T, edges: &Vec<(T, T)>) -> Vec<(T, usize)>
where
    T: Clone + Hash + Eq + std::fmt::Debug,
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

    let mut roots = vec![*to_graph_node.get(root).unwrap()];
    let mut depth: usize = 0;
    let mut visited = HashSet::new();
    while !roots.is_empty() {
        let mut new_roots = vec![];
        for &r in &roots {
            if !visited.contains(&r) {
                graph_depths.push((r.clone(), depth));
                visited.insert(r);

                new_roots.extend(graph.neighbors(r));
            }
        }

        roots = new_roots;
        depth += 1;
    }

    // convert graph nodes to real nodes
    graph_depths
        .iter()
        .map(|(n, d)| (graph.node_weight(*n).unwrap().clone(), *d))
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use crate::get_depths;

    #[test]
    fn test_get_depths() {
        assert_eq!(
            get_depths(&(vec![(0, 1), (1, 2), (2, 3)])),
            vec![(0, vec![(0, 0), (1, 1), (2, 2), (3, 3),])]
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

        assert!(depths.contains(&(0, vec![(0, 0), (1, 1), (2, 2), (3, 3),])));
        assert!(depths.contains(&(10, vec![(10, 0), (11, 1), (12, 2), (13, 3),])));
    }

    #[test]
    fn test_get_depths_loop() {
        assert_eq!(
            get_depths(&(vec![(0, 1), (0, 2), (1, 2), (2, 1)])),
            vec![(0, vec![(0, 0), (2, 1), (1, 1),])]
        );
    }

    #[test]
    fn test_top_level_loop() {
        assert_eq!(get_depths(&(vec![(0, 1), (1, 0)])), vec![]);
    }
}
