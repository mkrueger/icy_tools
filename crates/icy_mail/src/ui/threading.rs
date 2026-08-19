use std::collections::HashMap;

use crate::qwk::MessageInfo;

/// A message as it appears in the list, carrying its indentation inside a thread.
#[derive(Clone, Copy)]
pub struct Row {
    /// Index into `QwkPackage::infos` / `descriptors`.
    pub index: usize,
    /// Nesting level; `0` for a thread root or a flat-list entry.
    pub depth: u16,
    /// Whether this row starts a thread that has replies.
    pub has_children: bool,
}

/// Groups messages into reply threads.
///
/// QWK carries a `ref_msg_number` back-pointer, but many BBS packets leave it empty, so
/// messages that reference nothing fall back to grouping by normalized subject.
/// Threads are emitted depth-first, roots ordered by their newest message.
pub fn build_threads(infos: &[&MessageInfo]) -> Vec<Row> {
    let by_number: HashMap<u32, usize> = infos
        .iter()
        .enumerate()
        .filter(|(_, info)| info.number != 0)
        .map(|(pos, info)| (info.number, pos))
        .collect();

    // First message seen per subject, used when `ref_msg_number` is missing.
    let mut subject_root: HashMap<&str, usize> = HashMap::new();
    for (pos, info) in infos.iter().enumerate() {
        if !info.subject_key.is_empty() {
            subject_root.entry(info.subject_key.as_str()).or_insert(pos);
        }
    }

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); infos.len()];
    let mut roots: Vec<usize> = Vec::new();

    for (pos, info) in infos.iter().enumerate() {
        let parent = by_number
            .get(&info.ref_number)
            .copied()
            .or_else(|| subject_root.get(info.subject_key.as_str()).copied())
            .filter(|parent| *parent != pos);

        match parent {
            Some(parent) => children[parent].push(pos),
            None => roots.push(pos),
        }
    }

    break_cycles(&mut children, &mut roots, infos.len());

    // Newest activity first, so live discussions stay at the top.
    let newest: Vec<i64> = (0..infos.len()).map(|pos| subtree_newest(pos, &children, infos)).collect();
    roots.sort_by(|a, b| newest[*b].cmp(&newest[*a]).then(infos[*a].number.cmp(&infos[*b].number)));
    for list in &mut children {
        list.sort_by_key(|pos| (infos[*pos].date, infos[*pos].number));
    }

    let mut rows = Vec::with_capacity(infos.len());
    for root in roots {
        push_subtree(root, 0, &children, infos, &mut rows);
    }
    rows
}

/// Re-parents any node that cannot reach a root, so a malformed packet cannot hide messages.
fn break_cycles(children: &mut [Vec<usize>], roots: &mut Vec<usize>, len: usize) {
    let mut reachable = vec![false; len];
    let mut stack: Vec<usize> = roots.clone();
    while let Some(pos) = stack.pop() {
        if std::mem::replace(&mut reachable[pos], true) {
            continue;
        }
        stack.extend(children[pos].iter().copied());
    }

    for pos in 0..len {
        if !reachable[pos] {
            reachable[pos] = true;
            roots.push(pos);
            let mut stack = vec![pos];
            while let Some(cur) = stack.pop() {
                for child in &children[cur] {
                    if !reachable[*child] {
                        reachable[*child] = true;
                        stack.push(*child);
                    }
                }
            }
        }
    }

    // Drop child links pointing at nodes promoted to roots.
    let promoted: std::collections::HashSet<usize> = roots.iter().copied().collect();
    for (parent, list) in children.iter_mut().enumerate() {
        list.retain(|child| !promoted.contains(child) || *child == parent);
    }
}

fn subtree_newest(pos: usize, children: &[Vec<usize>], infos: &[&MessageInfo]) -> i64 {
    let mut newest = infos[pos].date.and_utc().timestamp();
    let mut stack = children[pos].clone();
    while let Some(cur) = stack.pop() {
        newest = newest.max(infos[cur].date.and_utc().timestamp());
        stack.extend(children[cur].iter().copied());
    }
    newest
}

fn push_subtree(pos: usize, depth: u16, children: &[Vec<usize>], infos: &[&MessageInfo], rows: &mut Vec<Row>) {
    rows.push(Row {
        index: infos[pos].index,
        depth,
        has_children: !children[pos].is_empty(),
    });
    for child in &children[pos] {
        push_subtree(*child, depth + 1, children, infos, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qwk::normalize_subject;

    fn info(index: usize, number: u32, ref_number: u32, subject: &str, minute: u32) -> MessageInfo {
        MessageInfo {
            index,
            number,
            ref_number,
            conference: 0,
            from: String::new(),
            to: String::new(),
            subject: subject.to_string(),
            subject_key: normalize_subject(subject),
            date: chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap().and_hms_opt(0, minute, 0).unwrap(),
            date_str: String::new(),
            lines: 0,
            private: false,
        }
    }

    fn rows_of(infos: &[MessageInfo]) -> Vec<Row> {
        build_threads(&infos.iter().collect::<Vec<_>>())
    }

    #[test]
    fn normalize_strips_reply_prefixes() {
        assert_eq!(normalize_subject("Re: Re: Hello"), "hello");
        assert_eq!(normalize_subject("RE[2]: Hello"), "hello");
        assert_eq!(normalize_subject("Fwd: Hello"), "hello");
        assert_eq!(normalize_subject("Hello"), "hello");
        assert_eq!(normalize_subject("Retro computing"), "retro computing");
    }

    #[test]
    fn ref_number_builds_a_chain() {
        let infos = vec![info(0, 10, 0, "Topic", 0), info(1, 11, 10, "Re: Topic", 1), info(2, 12, 11, "Re: Topic", 2)];
        let rows = rows_of(&infos);
        assert_eq!(rows.iter().map(|r| (r.index, r.depth)).collect::<Vec<_>>(), vec![(0, 0), (1, 1), (2, 2)]);
        assert!(rows[0].has_children);
        assert!(!rows[2].has_children);
    }

    #[test]
    fn missing_ref_falls_back_to_subject() {
        let infos = vec![info(0, 10, 0, "Topic", 0), info(1, 11, 0, "Re: Topic", 1)];
        let rows = rows_of(&infos);
        assert_eq!(rows.iter().map(|r| (r.index, r.depth)).collect::<Vec<_>>(), vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn unrelated_subjects_stay_separate_roots() {
        let infos = vec![info(0, 10, 0, "One", 0), info(1, 11, 0, "Two", 1)];
        let rows = rows_of(&infos);
        assert!(rows.iter().all(|r| r.depth == 0));
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn every_message_appears_exactly_once_despite_cycles() {
        // 10 -> 11 -> 10 is a cycle; nothing may be swallowed.
        let infos = vec![info(0, 10, 11, "A", 0), info(1, 11, 10, "B", 1), info(2, 12, 0, "C", 2)];
        let rows = rows_of(&infos);
        let mut seen: Vec<usize> = rows.iter().map(|r| r.index).collect();
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[test]
    fn newest_thread_is_listed_first() {
        let infos = vec![info(0, 10, 0, "Old", 0), info(1, 11, 0, "New", 5)];
        let rows = rows_of(&infos);
        assert_eq!(rows[0].index, 1);
    }
}
