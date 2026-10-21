use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::qwk::MessageInfo;

/// A message as it appears in the list, carrying its place inside a thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// Index into `QwkPackage::infos` / `descriptors`.
    pub index: usize,
    /// Nesting level; `0` for a thread root or a flat-list entry.
    pub depth: u16,
    /// Whether this row starts a thread that has replies.
    pub has_children: bool,
    /// Whether no sibling follows this row, so its tree connector ends here.
    pub last: bool,
    /// Bit `n` is set when the ancestor at depth `n + 1` has a following sibling,
    /// so that ancestor's tree line continues through this row.
    pub guides: u32,
    /// Replies below this row at any depth; they follow it directly in thread order.
    pub descendants: u32,
}

impl Row {
    /// An entry of the flat, unthreaded list.
    pub fn flat(index: usize) -> Self {
        Self {
            index,
            depth: 0,
            has_children: false,
            last: true,
            guides: 0,
            descendants: 0,
        }
    }
}

/// Groups messages into reply threads.
///
/// Numeric back-pointers take precedence, followed by FidoNet MSGID/REPLY linkage and
/// normalized subjects. All links stay within their conference.
/// Threads are emitted depth-first, roots ordered by their newest message.
pub fn build_threads(infos: &[&MessageInfo]) -> Vec<Row> {
    let by_number: HashMap<(u16, u32), usize> = infos
        .iter()
        .enumerate()
        .filter(|(_, info)| info.number != 0)
        .map(|(pos, info)| ((info.conference, info.number), pos))
        .collect();

    let mut by_id = HashMap::new();
    let mut subject_root: HashMap<(u16, &str), usize> = HashMap::new();
    for (pos, info) in infos.iter().enumerate() {
        if let Some(id) = info.ftn.msgid.as_deref() {
            let entry = by_id.entry((info.conference, id)).or_insert(Some(pos));
            if *entry != Some(pos) {
                log::warn!(
                    "Duplicate FidoNet MSGID {:?} in conference {}; ignoring ambiguous ID links",
                    String::from_utf8_lossy(id),
                    info.conference
                );
                *entry = None;
            }
        }
        if !info.subject_key.is_empty() {
            subject_root.entry((info.conference, info.subject_key.as_str())).or_insert(pos);
        }
    }

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); infos.len()];
    let mut roots: Vec<usize> = Vec::new();

    for (pos, info) in infos.iter().enumerate() {
        let parent = by_number
            .get(&(info.conference, info.ref_number))
            .copied()
            .filter(|parent| *parent != pos)
            .or_else(|| {
                info.ftn
                    .reply
                    .as_deref()
                    .and_then(|id| by_id.get(&(info.conference, id)).copied().flatten())
                    .filter(|parent| *parent != pos)
            })
            .or_else(|| {
                subject_root
                    .get(&(info.conference, info.subject_key.as_str()))
                    .copied()
                    .filter(|parent| *parent != pos)
            });

        match parent {
            Some(parent) => children[parent].push(pos),
            None => roots.push(pos),
        }
    }

    break_cycles(&mut children, &mut roots, infos.len());

    // Parents before children; walked backwards it yields each subtree's newest message in one pass.
    let mut order = Vec::with_capacity(infos.len());
    let mut stack = roots.clone();
    while let Some(pos) = stack.pop() {
        order.push(pos);
        stack.extend(children[pos].iter().copied());
    }
    let mut newest: Vec<i64> = infos.iter().map(|info| info.date.and_utc().timestamp()).collect();
    let mut descendants = vec![0u32; infos.len()];
    for &pos in order.iter().rev() {
        for &child in &children[pos] {
            newest[pos] = newest[pos].max(newest[child]);
            descendants[pos] = descendants[pos].saturating_add(descendants[child].saturating_add(1));
        }
    }

    // Newest activity first, so live discussions stay at the top.
    roots.sort_by(|a, b| newest[*b].cmp(&newest[*a]).then(infos[*a].number.cmp(&infos[*b].number)));
    for list in &mut children {
        list.sort_by_key(|pos| (infos[*pos].date, infos[*pos].number));
    }

    // Iterative, so very long reply chains cannot overflow the stack.
    let mut rows = Vec::with_capacity(infos.len());
    let mut stack: Vec<(usize, u16, u32, bool)> = roots.iter().rev().map(|root| (*root, 0, 0, true)).collect();
    while let Some((pos, depth, guides, last)) = stack.pop() {
        rows.push(Row {
            index: infos[pos].index,
            depth,
            has_children: !children[pos].is_empty(),
            last,
            guides,
            descendants: descendants[pos],
        });
        // This row's own connector keeps running past its replies while a sibling follows it.
        let child_guides = match depth.checked_sub(1) {
            Some(bit) if !last && bit < u32::BITS as u16 => guides | 1 << bit,
            _ => guides,
        };
        let count = children[pos].len();
        stack.extend(
            children[pos]
                .iter()
                .enumerate()
                .rev()
                .map(|(nth, child)| (*child, depth.saturating_add(1), child_guides, nth + 1 == count)),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qwk::normalize_subject;

    fn info(index: usize, number: u32, ref_number: u32, subject: &str, minute: u32) -> MessageInfo {
        MessageInfo {
            index,
            number,
            ref_number,
            ftn: crate::ftn::Metadata::default(),
            conference: 0,
            from: Default::default(),
            to: Default::default(),
            subject: subject.into(),
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
    fn message_ids_link_changed_subjects_and_out_of_order_messages() {
        let mut infos = vec![info(0, 11, 999, "Changed subject", 1), info(1, 10, 0, "Original", 0)];
        infos[0].ftn = crate::ftn::Metadata::parse(b"\x01REPLY: \"other network\" 1234ABCD\r");
        infos[1].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: \"other network\" 1234ABCD\r");
        let rows = rows_of(&infos);
        assert_eq!(rows.iter().map(|r| (r.index, r.depth)).collect::<Vec<_>>(), [(1, 0), (0, 1)]);
    }

    #[test]
    fn numeric_links_take_precedence_and_self_links_allow_id_fallback() {
        let mut infos = vec![info(0, 10, 0, "Numeric parent", 0), info(1, 11, 0, "ID parent", 1), info(2, 12, 10, "Reply", 2)];
        infos[1].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: 2:1/1 12345678\n");
        infos[2].ftn = crate::ftn::Metadata::parse(b"\x01REPLY: 2:1/1 12345678\n");
        let rows = rows_of(&infos);
        let parent = rows.iter().position(|row| row.index == 0).unwrap();
        assert_eq!((rows[parent + 1].index, rows[parent + 1].depth), (2, 1));

        infos[2].ref_number = 12;
        let rows = rows_of(&infos);
        let parent = rows.iter().position(|row| row.index == 1).unwrap();
        assert_eq!((rows[parent + 1].index, rows[parent + 1].depth), (2, 1));
    }

    #[test]
    fn links_and_subjects_never_cross_conferences() {
        let mut infos = vec![info(0, 10, 0, "Topic", 0), info(1, 11, 10, "Topic", 1)];
        infos[0].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: 2:1/1 12345678\n");
        infos[1].ftn = crate::ftn::Metadata::parse(b"\x01REPLY: 2:1/1 12345678\n");
        infos[1].conference = 1;
        assert!(rows_of(&infos).iter().all(|row| row.depth == 0));
    }

    #[test]
    fn duplicate_ids_are_ambiguous_and_id_comparison_is_exact() {
        let mut infos = vec![info(0, 10, 0, "First", 0), info(1, 11, 0, "Second", 1), info(2, 12, 0, "Reply", 2)];
        infos[0].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: 2:1/1 1234abcd\n");
        infos[1].ftn = infos[0].ftn.clone();
        infos[2].ftn = crate::ftn::Metadata::parse(b"\x01REPLY: 2:1/1 1234abcd\n");
        assert!(rows_of(&infos).iter().all(|row| row.depth == 0));
        infos[1].ftn = crate::ftn::Metadata::default();
        infos[2].ftn = crate::ftn::Metadata::parse(b"\x01REPLY: 2:1/1 1234ABCD\n");
        assert!(rows_of(&infos).iter().all(|row| row.depth == 0));
    }

    #[test]
    fn id_cycles_and_self_replies_keep_every_message_once() {
        let mut infos = vec![info(0, 10, 0, "First", 0), info(1, 11, 0, "Second", 1), info(2, 12, 0, "Self", 2)];
        infos[0].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: 2:1/1 00000001\n\x01REPLY: 2:1/1 00000002\n");
        infos[1].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: 2:1/1 00000002\n\x01REPLY: 2:1/1 00000001\n");
        infos[2].ftn = crate::ftn::Metadata::parse(b"\x01MSGID: 2:1/1 00000003\n\x01REPLY: 2:1/1 00000003\n");
        let mut seen: Vec<_> = rows_of(&infos).iter().map(|row| row.index).collect();
        seen.sort_unstable();
        assert_eq!(seen, [0, 1, 2]);
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
    fn long_reply_chains_are_threaded_without_recursion() {
        let infos: Vec<_> = (0..200_000u32).map(|pos| info(pos as usize, pos + 1, pos, "Chain", 0)).collect();
        let rows = rows_of(&infos);
        assert_eq!(rows.len(), infos.len());
        assert_eq!(rows.last().map(|row| (row.index, row.depth)), Some((199_999, u16::MAX)));
    }

    #[test]
    fn newest_thread_is_listed_first() {
        let infos = vec![info(0, 10, 0, "Old", 0), info(1, 11, 0, "New", 5)];
        let rows = rows_of(&infos);
        assert_eq!(rows[0].index, 1);
    }

    #[test]
    fn rows_carry_tree_connectors_and_subtree_sizes() {
        // 10
        // ├─ 11
        // │  └─ 13
        // └─ 12
        //    └─ 14
        let infos = vec![
            info(0, 10, 0, "Topic", 0),
            info(1, 11, 10, "Re: Topic", 1),
            info(2, 12, 10, "Re: Topic", 2),
            info(3, 13, 11, "Re: Topic", 3),
            info(4, 14, 12, "Re: Topic", 4),
        ];
        let rows = rows_of(&infos);
        assert_eq!(
            rows.iter().map(|r| (r.index, r.depth, r.last, r.guides, r.descendants)).collect::<Vec<_>>(),
            vec![
                (0, 0, true, 0, 4),
                (1, 1, false, 0, 1),
                (3, 2, true, 0b1, 0),
                (2, 1, true, 0, 1),
                (4, 2, true, 0, 0)
            ]
        );
    }
}
