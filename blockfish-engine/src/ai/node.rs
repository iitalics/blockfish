use super::{
    place::Place,
    score::{eval, penalty, ScoreParams},
    Snapshot,
};
use crate::{BasicMatrix, Color};

/// Search node.
#[derive(Clone)]
pub struct Node {
    state: State,
    score: i64,
    penalty: i64,
    trace: Vec<u8>,
}

impl Node {
    /// Constructs a new root node from state `state`. The root index is set to an invalid
    /// valid, so you need to use `.successors()` to set it.
    ///
    /// Note: the score is not calculated initially, since this node will most likely be
    /// immediately used to derive new nodes; the initial score would be discarded
    /// anyways.
    pub fn new(state: State) -> Self {
        Self {
            state,
            score: std::i64::MAX,
            penalty: 0,
            trace: Vec::with_capacity(8),
        }
    }

    // just accessors

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn score(&self) -> i64 {
        self.score
    }

    pub fn penalty(&self) -> i64 {
        self.penalty
    }

    pub fn depth(&self) -> usize {
        self.trace.len()
    }

    pub fn trace<'a>(&'a self) -> impl Iterator<Item = usize> + 'a {
        self.trace.iter().map(|&idx| idx as usize)
    }

    /// Builds and returns a successor node derived from this node and the placement
    /// `place`, using `scoring` to score the returned node. `idx` is used to update the
    /// traceback.
    pub fn successor(&self, scoring: &ScoreParams, idx: usize, place: &Place) -> Self {
        assert!(idx < (std::u8::MAX as _));
        let mut succ = self.clone();
        succ.trace.push(idx as u8);
        succ.state.place(&place);
        succ.score = if succ.state.is_goal() {
            (succ.depth() as i64) - 1000
        } else {
            eval(&succ.state.matrix).score(scoring)
        };
        succ.penalty = penalty(scoring, succ.depth());
        succ
    }
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "depth {}, score {:>3}, trace {:?}",
            self.depth(),
            self.score,
            self.trace,
        )
    }
}

/// Board state of a node.
#[derive(Clone)]
pub struct State {
    matrix: BasicMatrix,
    queue_rev: Vec<Color>,
    has_held: bool,
    is_goal: bool,
}

impl State {
    pub fn matrix(&self) -> &BasicMatrix {
        &self.matrix
    }

    pub fn is_goal(&self) -> bool {
        self.is_goal
    }

    /// Returns `true` if this state is at the max depth, so no further placements are
    /// possible.
    #[cfg(test)]
    pub fn is_max_depth(&self) -> bool {
        self.queue_rev.is_empty()
    }

    /// Returns `(next_piece, hold_piece)`, where either may be `None` if not available.
    ///
    /// Note: `hold_piece` is not exactly the current piece in hold, rather the piece you
    /// will get if you press hold, i.e. if hold is currently empty then it refers to the
    /// 2nd piece in the queue.
    pub fn next(&self) -> (Option<Color>, Option<Color>) {
        let from_top = |i| {
            self.queue_rev
                .len()
                .checked_sub(i)
                .and_then(|i| self.queue_rev.get(i))
                .cloned()
        };
        let c1 = from_top(1);
        let c2 = from_top(2);
        if self.has_held {
            (c2, c1)
        } else {
            (c1, c2)
        }
    }

    /// Applies the given placement to this state, modifying the queue and matrix.
    pub fn place(&mut self, pl: &Place) {
        pl.shape.blit_to(&mut self.matrix, pl.tf);
        self.is_goal = self.matrix.sift_rows();
        self.pop(pl.did_hold);
    }

    /// Removes a piece from the next queue, or hold slot if `hold` is `true`.
    fn pop(&mut self, hold: bool) {
        //  | has_held | hold  | pos
        // -+----------+-------+-----
        //  | true     | false | 2
        //  | true     | true  | 1
        //  | false    | false | 1
        //  | false    | true  | 2
        let pos = if self.has_held == hold { 1 } else { 2 };
        self.queue_rev.remove(self.queue_rev.len() - pos);
        self.has_held |= hold;
    }
}

impl From<Snapshot> for State {
    fn from(snapshot: Snapshot) -> Self {
        let matrix = snapshot.matrix;
        // the queue is represented in reverse order, so the next item can easily be
        // removed. the hold piece (if any) is stored on top, after the previews.
        let mut queue_rev = snapshot.queue;
        queue_rev.reverse();
        let mut has_held = false;
        if let Some(hold_color) = snapshot.hold {
            has_held = true;
            queue_rev.push(hold_color);
        }
        Self {
            matrix,
            queue_rev,
            has_held,
            is_goal: false,
        }
    }
}

//////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    use super::*;
    use crate::{basic_matrix, shape::srs, Orientation::*};

    #[test]
    fn test_state_operations() {
        let queue = || "LTJI".chars().map(Color::n);

        let mut s: State = Snapshot {
            hold: None,
            queue: queue().collect(),
            matrix: BasicMatrix::with_cols(10),
        }
        .into();
        assert!(!s.is_max_depth());
        assert_eq!(s.matrix.rows(), 0);
        assert_eq!(s.matrix.cols(), 10);
        assert_eq!(s.next(), (Some(Color::n('L')), Some(Color::n('T'))));

        let srs = srs();
        for (i, color) in queue().enumerate() {
            let shape = srs.shape(color).unwrap();
            let tf = ((i * 2) as i16 - 1, 0, R0);
            s.place(&Place::new(shape, tf, false));
        }
        assert!(s.is_max_depth());
        assert_eq!(s.matrix.rows(), 7);
        assert_eq!(s.next(), (None, None));
    }

    #[test]
    fn test_state_use_hold() {
        // something already in hold
        let mut s: State = Snapshot {
            hold: Some(Color::n('S')),
            queue: "LTJI".chars().map(Color::n).collect(),
            matrix: BasicMatrix::with_cols(10),
        }
        .into();
        assert_eq!(s.next(), (Some(Color::n('L')), Some(Color::n('S'))));
        s.pop(true);
        assert_eq!(s.next(), (Some(Color::n('T')), Some(Color::n('L'))));
        s.pop(false);
        assert_eq!(s.next(), (Some(Color::n('J')), Some(Color::n('L'))));
        // nothing previously in hold
        s = Snapshot {
            hold: None,
            queue: "LTJI".chars().map(Color::n).collect(),
            matrix: BasicMatrix::with_cols(10),
        }
        .into();
        assert_eq!(s.next(), (Some(Color::n('L')), Some(Color::n('T'))));
        s.pop(true);
        assert_eq!(s.next(), (Some(Color::n('J')), Some(Color::n('L'))));
    }

    #[test]
    fn test_state_nearly_empty_queue() {
        let mut s: State = Snapshot {
            hold: None,
            queue: vec![Color::n('I')],
            matrix: BasicMatrix::with_cols(10),
        }
        .into();
        assert_eq!(s.next(), (Some(Color::n('I')), None));
        s = Snapshot {
            hold: Some(Color::n('O')),
            queue: vec![],
            matrix: BasicMatrix::with_cols(10),
        }
        .into();
        assert_eq!(s.next(), (None, Some(Color::n('O'))));
    }

    #[test]
    fn test_node_successor() {
        let srs = srs();
        let sp = ScoreParams::default();
        let (xx, __) = (true, false);

        // x . . . .
        // x x . . .
        let matrix = basic_matrix![[xx, xx, __, __, __], [xx, __, __, __, __]];
        let mut node = Node::new(
            Snapshot {
                hold: None,
                queue: vec![Color::n('L'), Color::n('O')],
                matrix,
            }
            .into(),
        );
        assert_eq!(node.depth(), 0);
        assert_eq!(node.trace().count(), 0);
        assert_eq!(node.state.is_goal(), false);

        // x . . . L
        // x x L L L  ==>  x . . . L
        let l = srs.shape(Color::n('L')).unwrap();
        let tf = (-1, 2, R0);
        node = node.successor(&sp, 3, &Place::new(l, tf, false));
        assert_eq!(node.depth(), 1);
        assert_eq!(node.state.next().0, Some(Color::n('O')));
        assert_eq!(node.state.next().1, None);
        assert_eq!(node.state.matrix(), &basic_matrix![[xx, __, __, __, xx]]);
        assert_eq!(node.trace().collect::<Vec<_>>(), [3]);
        assert_eq!(node.state.is_goal(), true);

        // O O . . .
        // O O . . .
        // x . . . L
        let o = srs.shape(Color::n('O')).unwrap();
        let tf = (0, -1, R0);
        node = node.successor(&sp, 4, &Place::new(o, tf, false));
        assert_eq!(node.depth(), 2);
        assert!(node.state.is_max_depth());
        assert_eq!(node.trace().collect::<Vec<_>>(), [3, 4]);
        assert_eq!(
            node.state.matrix,
            basic_matrix![
                [xx, __, __, __, xx],
                [xx, xx, __, __, __],
                [xx, xx, __, __, __],
            ]
        );
        assert_eq!(node.state.is_goal(), false);
    }
}
