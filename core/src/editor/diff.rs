use std::cmp::max;

#[derive(Debug, PartialEq)]
pub enum Diff {
    Left(usize),
    Both(usize),
    Right(usize),
}

fn precompute(b1: &[u8], b2: &[u8]) -> (usize, Option<usize>) {
    let l1 = b1.len();
    let l2 = b2.len();

    let mut start = 0;
    while start < l1 && start < l2 && b1[start] == b2[start] {
        start += 1;
    }

    let mut end = 0;
    while start < l1 - end && start < l2 - end && b1[l1 - 1 - end] == b2[l2 - 1 - end] {
        end += 1;
    }

    if start == end || end == 0 {
        (start, None)
    } else {
        (start, Some(end))
    }
}

fn find_lcs(left: &[u8], right: &[u8]) -> Vec<u8> {
    let el = left.len();
    let er = right.len();

    let mut mt = vec![0; el * er];
    for i in 0..el {
        for j in 0..er {
            mt[i * er + j] = if left[i] == right[j] {
                if i == 0 || j == 0 {
                    1
                } else {
                    mt[(i - 1) * er + j - 1] + 1
                }
            } else if i == 0 {
                if j == 0 {
                    0
                } else {
                    mt[i * er + j - 1]
                }
            } else if j == 0 {
                mt[(i - 1) * er + j]
            } else {
                max(mt[i * er + j - 1], mt[(i - 1) * er + j])
            };
        }
    }

    let mut lcs = Vec::new();
    let mut i = (el - 1) as isize;
    let mut j = (er - 1) as isize;
    while i >= 0 && j >= 0 {
        let idx = i as usize;
        let jdx = j as usize;
        if left[idx] == right[jdx] {
            lcs.insert(0, left[idx]);
            i -= 1;
            j -= 1;
        } else if j == 0 && i == 0 {
            break;
        } else if i == 0 || mt[idx * er + jdx - 1] > mt[(idx - 1) * er + jdx] {
            j -= 1;
        } else {
            i -= 1;
        }
    }

    lcs
}

#[allow(clippy::many_single_char_names)]
pub fn diff(left: &str, right: &str) -> Vec<Diff> {
    let bleft = left.as_bytes();
    let bright = right.as_bytes();
    let mut diffs = Vec::new();

    let (start, maybe_end) = precompute(bleft, bright);
    if start > 0 {
        diffs.push(Diff::Both(start));
    }
    let end = match maybe_end {
        Some(len) => len,
        None if start > 0 => return diffs,
        None => 0,
    };

    let l = &bleft[start..bleft.len() - end];
    let r = &bright[start..bright.len() - end];
    let lcs = find_lcs(l, r);
    let mut i = 0;
    let mut j = 0;
    let mut len;

    let mut lcs_it = lcs.iter();
    while let Some(&c) = lcs_it.next() {
        len = 0;
        while i < l.len() && l[i] != c {
            len += 1;
            i += 1;
        }
        if len > 0 {
            diffs.push(Diff::Left(len));
        }

        len = 0;
        while j < r.len() && r[j] != c {
            len += 1;
            j += 1;
        }
        if len > 0 {
            diffs.push(Diff::Right(len));
        }

        len = 0;
        let mut sub_c = c;
        while l[i] == sub_c && r[j] == sub_c {
            len += 1;
            i += 1;
            j += 1;
            match lcs_it.next() {
                Some(&v) => sub_c = v,
                None => break,
            }
        }
        if len > 0 {
            diffs.push(Diff::Both(len));
        }
    }

    if i < l.len() - 1 {
        diffs.push(Diff::Left(l.len() - i));
    }
    if j < r.len() - 1 {
        diffs.push(Diff::Right(r.len() - j));
    }

    if end > 0 {
        diffs.push(Diff::Both(end));
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcs() {
        assert_eq!(find_lcs(b"toto", b"toto"), b"toto");
        assert_eq!(find_lcs(b"toto", b"titola"), b"tto");
        assert_eq!(find_lcs(b"toto", b"tata"), b"tt");
        assert_eq!(
            find_lcs(
                b"the quick brown fox jumps over the lazy dog",
                b"the sneaky fox jumps over the migthy bear"
            ),
            b"the k fox jumps over the y "
        );
    }

    fn assert_diff(left: &str, right: &str, diffs: Vec<Diff>) {
        let mut rebuilt = String::from(left);
        let mut loffset = 0;
        let mut roffset = 0;
        for diff in diffs {
            match diff {
                Diff::Left(len) => rebuilt.replace_range(loffset..loffset + len, ""),
                Diff::Right(len) => {
                    rebuilt.insert_str(loffset, &right[roffset..roffset + len]);
                    loffset += len;
                    roffset += len;
                }
                Diff::Both(len) => {
                    loffset += len;
                    roffset += len;
                }
            }
        }
        assert_eq!(rebuilt, right);
    }

    #[test]
    fn close_diff() {
        let left = "🦊 the quick brown fox jumps over the lazy dog 🐶, so quick";
        let right = "🦊 the sneaky fox jumps over the mighty bear 🐶, so quick";
        let diffs = diff(left, right);

        let left_len: usize = diffs
            .iter()
            .filter_map(|d| match d {
                Diff::Both(len) | Diff::Left(len) => Some(len),
                _ => None,
            })
            .sum();
        let right_len: usize = diffs
            .iter()
            .filter_map(|d| match d {
                Diff::Both(len) | Diff::Right(len) => Some(len),
                _ => None,
            })
            .sum();
        assert_eq!(left_len, left.len());
        assert_eq!(right_len, right.len());
        assert_diff(left, right, diffs);
    }

    #[test]
    fn far_diff() {
        let left = "the really quick brown fox jumps over the lazy dog 🐶, so fast";
        let right = "nothing to see here";
        let diffs = diff(left, right);
        assert_diff(left, right, diffs);
    }
}
