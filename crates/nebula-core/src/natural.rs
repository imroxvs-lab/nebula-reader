//! Orden natural de cadenas: `page2` va antes que `page10`.

use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::CharIndices;

/// Compara dos cadenas en orden natural.
///
/// - Las secuencias de dígitos se comparan por su valor numérico, sin límite
///   de longitud (`007` = `7` < `10`).
/// - El resto de caracteres se compara sin distinguir mayúsculas.
/// - Si todo coincide, se desempata con la comparación de la cadena original
///   para que el orden sea total y estable (`a01` < `a1`, `A` < `a`).
///
/// ```
/// use std::cmp::Ordering;
/// use nebula_core::natural::natural_cmp;
///
/// assert_eq!(natural_cmp("page2.jpg", "page10.jpg"), Ordering::Less);
/// assert_eq!(natural_cmp("Cap 3", "cap 3"), Ordering::Less);
/// ```
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut left = Chunks::new(a);
    let mut right = Chunks::new(b);
    loop {
        match (left.next(), right.next()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ordering = compare_chunks(x, y);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

fn compare_chunks(a: Chunk<'_>, b: Chunk<'_>) -> Ordering {
    match (a, b) {
        (Chunk::Digits(x), Chunk::Digits(y)) => compare_numbers(x, y),
        (Chunk::Text(x), Chunk::Text(y)) => compare_text(x, y),
        // Los números van antes que el texto, como en el Explorador de Windows.
        (Chunk::Digits(_), Chunk::Text(_)) => Ordering::Less,
        (Chunk::Text(_), Chunk::Digits(_)) => Ordering::Greater,
    }
}

/// Compara secuencias de dígitos ASCII por valor sin convertirlas a enteros,
/// para no desbordar con números muy largos.
fn compare_numbers(a: &str, b: &str) -> Ordering {
    let a = a.trim_start_matches('0');
    let b = b.trim_start_matches('0');
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn compare_text(a: &str, b: &str) -> Ordering {
    let lower_a = a.chars().flat_map(char::to_lowercase);
    let lower_b = b.chars().flat_map(char::to_lowercase);
    lower_a.cmp(lower_b)
}

#[derive(Debug, Clone, Copy)]
enum Chunk<'a> {
    Digits(&'a str),
    Text(&'a str),
}

/// Divide una cadena en tramos alternos de dígitos ASCII y de otro texto.
struct Chunks<'a> {
    source: &'a str,
    chars: Peekable<CharIndices<'a>>,
}

impl<'a> Chunks<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (start, first) = self.chars.next()?;
        let digits = first.is_ascii_digit();
        let mut end = start + first.len_utf8();
        while let Some(&(index, c)) = self.chars.peek() {
            if c.is_ascii_digit() != digits {
                break;
            }
            end = index + c.len_utf8();
            self.chars.next();
        }
        let slice = &self.source[start..end];
        Some(if digits {
            Chunk::Digits(slice)
        } else {
            Chunk::Text(slice)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sorted(mut items: Vec<&str>) -> Vec<&str> {
        items.sort_by(|a, b| natural_cmp(a, b));
        items
    }

    #[test]
    fn numbers_are_compared_by_value() {
        assert_eq!(
            sorted(vec!["p10.jpg", "p2.jpg", "p1.jpg"]),
            ["p1.jpg", "p2.jpg", "p10.jpg"]
        );
    }

    #[test]
    fn leading_zeros_do_not_change_value() {
        assert_eq!(
            sorted(vec!["010.jpg", "9.jpg", "001.jpg"]),
            ["001.jpg", "9.jpg", "010.jpg"]
        );
    }

    #[test]
    fn text_is_case_insensitive() {
        assert_eq!(
            sorted(vec!["b.jpg", "A.jpg", "c.jpg"]),
            ["A.jpg", "b.jpg", "c.jpg"]
        );
    }

    #[test]
    fn folders_are_ordered_naturally() {
        assert_eq!(
            sorted(vec![
                "cap10/01.jpg",
                "cap2/01.jpg",
                "cap2/10.jpg",
                "cap2/2.jpg"
            ]),
            ["cap2/01.jpg", "cap2/2.jpg", "cap2/10.jpg", "cap10/01.jpg"]
        );
    }

    #[test]
    fn huge_numbers_do_not_overflow() {
        assert_eq!(
            natural_cmp("a99999999999999999999999", "a100000000000000000000000"),
            Ordering::Less
        );
    }

    #[test]
    fn ties_are_broken_deterministically() {
        assert_eq!(natural_cmp("a01", "a1"), Ordering::Less);
    }

    proptest! {
        #[test]
        fn is_reflexive(a in "\\PC{0,20}") {
            prop_assert_eq!(natural_cmp(&a, &a), Ordering::Equal);
        }

        #[test]
        fn is_antisymmetric(a in "[a-zA-Z0-9 ._-]{0,12}", b in "[a-zA-Z0-9 ._-]{0,12}") {
            prop_assert_eq!(natural_cmp(&a, &b), natural_cmp(&b, &a).reverse());
        }

        #[test]
        fn equal_only_for_identical_strings(a in "\\PC{0,12}", b in "\\PC{0,12}") {
            prop_assert_eq!(natural_cmp(&a, &b) == Ordering::Equal, a == b);
        }

        #[test]
        fn is_transitive(
            a in "[aB0-9]{0,6}",
            b in "[aB0-9]{0,6}",
            c in "[aB0-9]{0,6}",
        ) {
            if natural_cmp(&a, &b) != Ordering::Greater && natural_cmp(&b, &c) != Ordering::Greater {
                prop_assert_ne!(natural_cmp(&a, &c), Ordering::Greater);
            }
        }

        #[test]
        fn numeric_suffix_orders_by_value(x in 0u64..100_000, y in 0u64..100_000) {
            let a = format!("page{x}.jpg");
            let b = format!("page{y}.jpg");
            prop_assert_eq!(natural_cmp(&a, &b), x.cmp(&y));
        }
    }
}
