use std::io::{Cursor, Seek};

use binrw::{BinRead, BinResult, Endian};

use derivative::Derivative;

#[derive(Debug, Default, PartialEq, Eq, Clone, BinRead)]
#[br(import(element_size: u32))]
pub struct QuadTree {
    #[br(map = |x: u8| x != 0)]
    flag: bool,

    #[br(args(flag, element_size))]
    root: QuadTreeData,
}

/// One leaf of a [`QuadTree`]: a square block of cells that share one 4-byte value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuadTreeSpan {
    /// Column of the block's first cell.
    pub x: u32,
    /// Row of the block's first cell (row 0 is the south edge).
    pub y: u32,
    /// Side length of the block in cells.
    pub size: u32,
    /// Raw 4-byte leaf value.
    pub data: [u8; 4],
}

impl QuadTreeSpan {
    /// Leaf value as a little-endian `u32` (the object and map-info offset trees).
    pub fn value_u32(&self) -> u32 {
        u32::from_le_bytes(self.data)
    }
}

impl QuadTree {
    /// Leaves in file order. Each node splits its block into 4 x 4 children, row by row.
    ///
    /// `root_size` is the side of the whole tree in cells: the smallest power of four that
    /// covers the grid (256 for a 256 x 256 layer). Spans are in units of one 4-byte leaf
    /// element, which is one grid cell for the 4-byte trees (`object_offsets`,
    /// `map_object_offsets`).
    pub fn leaves(&self, root_size: u32) -> Vec<QuadTreeSpan> {
        let mut out = Vec::new();
        self.root.collect_leaves(0, 0, root_size, &mut out);
        out
    }
}

impl QuadTreeData {
    fn collect_leaves(&self, x: u32, y: u32, size: u32, out: &mut Vec<QuadTreeSpan>) {
        match self {
            Self::Node(node) => {
                let child = (size / 4).max(1);
                for (i, sub) in (0_u32..).zip(&node.sub_trees) {
                    sub.collect_leaves(x + (i % 4) * child, y + (i / 4) * child, child, out);
                }
            }
            Self::Leaf(leaf) => {
                let mut data = [0_u8; 4];
                let n = leaf.data.len().min(4);
                data[..n].copy_from_slice(&leaf.data[..n]);
                out.push(QuadTreeSpan { x, y, size, data });
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, BinRead, Derivative)]
#[derivative(Default)]
#[br(import(flag: bool, element_size: u32))]
pub enum QuadTreeData {
    #[br(pre_assert(flag))]
    Node(#[br(args(element_size))] QuadTreeNode),
    #[br(pre_assert(!flag))]
    #[derivative(Default)]
    Leaf(#[br(args(element_size))] QuadTreeLeaf),
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct QuadTreeNode {
    sub_trees: Vec<QuadTreeData>,
}

impl BinRead for QuadTreeNode {
    type Args<'a> = (u32,);

    fn read_options<R: std::io::Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let mut bit_mask = u16::read_options(reader, endian, ())?;
        assert!(args.0 > 0);
        let mut sub_trees = Vec::with_capacity(16);
        for _ in 0..16 {
            if (bit_mask & 1) == 1 {
                sub_trees.push(QuadTreeData::Node(Self::read_options(
                    reader, endian, args,
                )?));
            } else {
                sub_trees.push(QuadTreeData::Leaf(QuadTreeLeaf::read_options(
                    reader, endian, args,
                )?));
            }
            bit_mask >>= 1;
        }

        Ok(Self { sub_trees })
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct QuadTreeLeaf {
    element_size: u32,
    data: Vec<u8>,
}

impl BinRead for QuadTreeLeaf {
    type Args<'a> = (u32,);

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let mut data = vec![0_u8; 4];
        reader.read_exact(&mut data)?;

        Ok(Self {
            element_size: args.0,
            data,
        })
    }
}

impl QuadTreeLeaf {
    pub fn get<'a, T: BinRead<Args<'a> = ()>>(&self, x: u32, y: u32) -> BinResult<Option<T>> {
        let offset = u64::from(match self.element_size {
            1 => 0,
            2 => x * 2,
            4 => (y << 1) + x,
            _ => todo!(),
        });

        let mut data_reader = Cursor::new(&self.data);

        if offset > data_reader.stream_position()? {
            return Ok(None);
        }

        data_reader.set_position(offset);

        let val = T::read_options(&mut data_reader, Endian::Little, ())?;
        Ok(Some(val))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_tree(bytes: &[u8]) -> QuadTree {
        QuadTree::read_options(&mut Cursor::new(bytes), Endian::Little, (4,)).unwrap()
    }

    fn leaf(v: u32) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    #[test]
    fn single_leaf_covers_the_whole_root() {
        let mut bytes = vec![0_u8];
        bytes.extend(leaf(7));
        let spans = read_tree(&bytes).leaves(256);
        assert_eq!(
            spans,
            vec![QuadTreeSpan {
                x: 0,
                y: 0,
                size: 256,
                data: 7_u32.to_le_bytes()
            }]
        );
    }

    #[test]
    fn children_go_row_by_row_and_nested_nodes_subdivide() {
        // Root node: child 5 (column 1, row 1) is a node, the other 15 are leaves.
        let mut bytes = vec![1_u8];
        bytes.extend((1_u16 << 5).to_le_bytes());
        for i in 0..16_u32 {
            if i == 5 {
                bytes.extend(0_u16.to_le_bytes());
                for k in 0..16 {
                    bytes.extend(leaf(100 + k));
                }
            } else {
                bytes.extend(leaf(i));
            }
        }
        let spans = read_tree(&bytes).leaves(64);
        assert_eq!(spans.len(), 31);
        assert_eq!((spans[1].x, spans[1].y, spans[1].size), (16, 0, 16));
        assert_eq!((spans[4].x, spans[4].y, spans[4].size), (0, 16, 16));
        let nested = &spans[5..21];
        assert_eq!((nested[0].x, nested[0].y, nested[0].size), (16, 16, 4));
        assert_eq!((nested[6].x, nested[6].y), (24, 20));
        assert_eq!(nested[15].value_u32(), 115);
        assert_eq!(
            (spans[21].x, spans[21].y, spans[21].value_u32()),
            (32, 16, 6)
        );
    }
}
