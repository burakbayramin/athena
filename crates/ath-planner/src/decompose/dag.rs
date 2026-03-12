//! Pure DAG algorithms for phase dependency graphs.
//!
//! All functions operate on `&[PhaseSpec]` slices and perform no I/O.

use std::collections::{HashMap, VecDeque};

use ath_types::PhaseSpec;

/// Topologically sort phases by their dependency edges (Kahn's algorithm).
///
/// Returns `Ok(sorted_ids)` on success, or `Err(cycle_ids)` if a cycle is detected.
pub fn topological_sort(phases: &[PhaseSpec]) -> Result<Vec<u32>, Vec<u32>> {
    let ids: Vec<u32> = phases.iter().map(|p| p.id).collect();

    // Build adjacency list and in-degree map
    let mut in_degree: HashMap<u32, usize> = ids.iter().map(|&id| (id, 0)).collect();
    let mut adjacency: HashMap<u32, Vec<u32>> = ids.iter().map(|&id| (id, Vec::new())).collect();

    for phase in phases {
        for &dep in &phase.depends_on {
            adjacency.entry(dep).or_default().push(phase.id);
            *in_degree.entry(phase.id).or_insert(0) += 1;
        }
    }

    // BFS from nodes with in-degree 0
    let mut queue: VecDeque<u32> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    // Sort the initial queue for deterministic output
    let mut initial: Vec<u32> = queue.drain(..).collect();
    initial.sort();
    queue.extend(initial);

    let mut sorted = Vec::with_capacity(phases.len());

    while let Some(node) = queue.pop_front() {
        sorted.push(node);
        if let Some(neighbors) = adjacency.get(&node) {
            let mut next_ready = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(&neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next_ready.push(neighbor);
                    }
                }
            }
            // Sort for deterministic output
            next_ready.sort();
            queue.extend(next_ready);
        }
    }

    if sorted.len() != phases.len() {
        Err(find_cycle_path(phases))
    } else {
        Ok(sorted)
    }
}

/// Find a cycle in the phase dependency graph using DFS color marking.
///
/// Returns a `Vec<u32>` of phase IDs forming the cycle.
fn find_cycle_path(phases: &[PhaseSpec]) -> Vec<u32> {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let dep_map: HashMap<u32, &[u32]> = phases
        .iter()
        .map(|p| (p.id, p.depends_on.as_slice()))
        .collect();

    let mut color: HashMap<u32, Color> = phases.iter().map(|p| (p.id, Color::White)).collect();
    let mut stack: Vec<u32> = Vec::new();
    let mut cycle: Vec<u32> = Vec::new();

    fn dfs(
        node: u32,
        dep_map: &HashMap<u32, &[u32]>,
        color: &mut HashMap<u32, Color>,
        stack: &mut Vec<u32>,
        cycle: &mut Vec<u32>,
    ) -> bool {
        color.insert(node, Color::Gray);
        stack.push(node);

        if let Some(&deps) = dep_map.get(&node) {
            for &dep in deps {
                match color.get(&dep) {
                    Some(Color::Gray) => {
                        // Found cycle -- extract from stack
                        let start = stack.iter().position(|&n| n == dep).unwrap();
                        *cycle = stack[start..].to_vec();
                        return true;
                    }
                    Some(Color::White) => {
                        if dfs(dep, dep_map, color, stack, cycle) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }

        stack.pop();
        color.insert(node, Color::Black);
        false
    }

    let mut ids: Vec<u32> = phases.iter().map(|p| p.id).collect();
    ids.sort();

    for &id in &ids {
        if color[&id] == Color::White {
            if dfs(id, &dep_map, &mut color, &mut stack, &mut cycle) {
                return cycle;
            }
        }
    }

    cycle
}

/// Compute parallel execution groups from a topologically sorted phase set.
///
/// Each group contains phases that can execute concurrently (same dependency depth level).
/// Phases at level 0 have no dependencies; level N phases depend only on phases at levels < N.
pub fn compute_parallel_groups(phases: &[PhaseSpec], sorted_order: &[u32]) -> Vec<Vec<u32>> {
    let mut level: HashMap<u32, usize> = HashMap::new();

    // Build a lookup for dependencies
    let dep_map: HashMap<u32, &Vec<u32>> = phases.iter().map(|p| (p.id, &p.depends_on)).collect();

    let mut max_level: usize = 0;
    for &id in sorted_order {
        let deps = dep_map.get(&id).map(|d| d.as_slice()).unwrap_or(&[]);
        let my_level = if deps.is_empty() {
            0
        } else {
            deps.iter()
                .filter_map(|d| level.get(d))
                .max()
                .map(|l| l + 1)
                .unwrap_or(0)
        };
        level.insert(id, my_level);
        if my_level > max_level {
            max_level = my_level;
        }
    }

    let mut groups: Vec<Vec<u32>> = vec![Vec::new(); max_level + 1];
    for &id in sorted_order {
        let l = level[&id];
        groups[l].push(id);
    }

    groups
}

/// Compute the critical path length (longest dependency chain) in the DAG.
///
/// Returns the length of the longest path. A single phase with no dependencies has length 1.
pub fn critical_path_length(phases: &[PhaseSpec], sorted_order: &[u32]) -> usize {
    let dep_map: HashMap<u32, &Vec<u32>> = phases.iter().map(|p| (p.id, &p.depends_on)).collect();
    let mut path_len: HashMap<u32, usize> = HashMap::new();

    let mut max_path: usize = 0;
    for &id in sorted_order {
        let deps = dep_map.get(&id).map(|d| d.as_slice()).unwrap_or(&[]);
        let my_len = if deps.is_empty() {
            1
        } else {
            deps.iter()
                .filter_map(|d| path_len.get(d))
                .max()
                .map(|l| l + 1)
                .unwrap_or(1)
        };
        path_len.insert(id, my_len);
        if my_len > max_path {
            max_path = my_len;
        }
    }

    max_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use ath_types::plan::TaskSpec;
    use ath_types::project::SkillTag;

    /// Helper to create a minimal PhaseSpec for testing.
    fn make_phase(id: u32, name: &str, depends_on: Vec<u32>) -> PhaseSpec {
        PhaseSpec {
            id,
            name: name.into(),
            description: format!("Phase {name}"),
            tasks: vec![TaskSpec {
                name: format!("{name} task"),
                description: "dummy".into(),
                skill_tags: vec![SkillTag("test".into())],
                expected_output_files: vec![],
                acceptance_criteria: vec![],
                goal_indices: vec![0],
            }],
            depends_on,
            produces: vec![],
            consumes: vec![],
        }
    }

    #[test]
    fn topo_sort_linear_chain() {
        // A -> B -> C
        let phases = vec![
            make_phase(1, "A", vec![]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![2]),
        ];
        let result = topological_sort(&phases).expect("should succeed");
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn topo_sort_diamond() {
        // A -> B, A -> C, B -> D, C -> D
        let phases = vec![
            make_phase(1, "A", vec![]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![1]),
            make_phase(4, "D", vec![2, 3]),
        ];
        let result = topological_sort(&phases).expect("should succeed");
        // A must come before B and C; B and C must come before D
        let pos = |id: u32| result.iter().position(|&x| x == id).unwrap();
        assert!(pos(1) < pos(2));
        assert!(pos(1) < pos(3));
        assert!(pos(2) < pos(4));
        assert!(pos(3) < pos(4));
    }

    #[test]
    fn topo_sort_cycle_detected() {
        // A -> B -> C -> A
        let phases = vec![
            make_phase(1, "A", vec![3]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![2]),
        ];
        let result = topological_sort(&phases);
        assert!(result.is_err());
        let cycle = result.unwrap_err();
        // Cycle should contain all three IDs
        assert!(cycle.contains(&1));
        assert!(cycle.contains(&2));
        assert!(cycle.contains(&3));
    }

    #[test]
    fn topo_sort_disconnected() {
        // A, B -- no dependencies
        let phases = vec![
            make_phase(1, "A", vec![]),
            make_phase(2, "B", vec![]),
        ];
        let result = topological_sort(&phases).expect("should succeed");
        assert_eq!(result.len(), 2);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
    }

    #[test]
    fn parallel_groups_diamond() {
        // A -> B, A -> C, B -> D, C -> D
        let phases = vec![
            make_phase(1, "A", vec![]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![1]),
            make_phase(4, "D", vec![2, 3]),
        ];
        let order = topological_sort(&phases).unwrap();
        let groups = compute_parallel_groups(&phases, &order);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0], vec![1]);
        assert!(groups[1].contains(&2) && groups[1].contains(&3));
        assert_eq!(groups[2], vec![4]);
    }

    #[test]
    fn parallel_groups_linear() {
        let phases = vec![
            make_phase(1, "A", vec![]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![2]),
        ];
        let order = topological_sort(&phases).unwrap();
        let groups = compute_parallel_groups(&phases, &order);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0], vec![1]);
        assert_eq!(groups[1], vec![2]);
        assert_eq!(groups[2], vec![3]);
    }

    #[test]
    fn critical_path_diamond() {
        let phases = vec![
            make_phase(1, "A", vec![]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![1]),
            make_phase(4, "D", vec![2, 3]),
        ];
        let order = topological_sort(&phases).unwrap();
        let cpl = critical_path_length(&phases, &order);
        assert_eq!(cpl, 3); // A->B->D or A->C->D
    }

    #[test]
    fn critical_path_single_phase() {
        let phases = vec![make_phase(1, "A", vec![])];
        let order = topological_sort(&phases).unwrap();
        let cpl = critical_path_length(&phases, &order);
        assert_eq!(cpl, 1);
    }

    #[test]
    fn find_cycle_path_returns_cycle_members() {
        // A -> B -> C -> A
        let phases = vec![
            make_phase(1, "A", vec![3]),
            make_phase(2, "B", vec![1]),
            make_phase(3, "C", vec![2]),
        ];
        let cycle = find_cycle_path(&phases);
        assert!(cycle.contains(&1));
        assert!(cycle.contains(&2));
        assert!(cycle.contains(&3));
    }
}
