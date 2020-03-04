//! Source file support for diagnostic reporting.

use std::ops::Range;
use std::sync::Arc;

/// A line within a source file.
pub struct Line {
    /// The line number.
    pub number: usize,
    /// The byte range of the line in the source.
    pub range: Range<usize>,
}

/// Files that can be used for pretty printing.
///
/// A lifetime parameter `'a` is provided to allow any of the returned values to returned by reference.
/// This is to workaround the lack of higher kinded lifetime parameters.
/// This can be ignored if this is not needed, however.
pub trait Files {
    type FileId: Copy + PartialEq;
    type Origin: std::fmt::Display;
    type Source: AsRef<str>;

    /// The origin of a file.
    fn origin(&self, id: Self::FileId) -> Option<Self::Origin>;

    /// The line at the given index.
    fn line(&self, id: Self::FileId, line_index: usize) -> Option<Line>;

    /// The index of the line at the given byte index.
    fn line_index(&self, id: Self::FileId, byte_index: usize) -> Option<usize>;

    /// The source of the file.
    fn source(&self, id: Self::FileId) -> Option<Self::Source>;
}

/// A single source file.
///
/// This is useful for simple language tests, but it might be worth creating a
/// custom implementation when a language scales beyond a certain size.
#[derive(Debug, Clone)]
pub struct SimpleFile<Origin> {
    /// The origin of the file.
    origin: Origin,
    /// The source code of the file.
    source: Arc<str>,
    /// The starting byte indices in the source code.
    line_starts: Vec<usize>,
}

/// Compute the line starts of a file.
pub fn line_starts<'a>(source: &'a str) -> impl 'a + Iterator<Item = usize> {
    std::iter::once(0).chain(source.match_indices('\n').map(|(i, _)| i + 1))
}

/// The column index at the given byte index in the source file.
/// This is the number of characters to the given byte index.
///
/// If the byte index is smaller than the start of the line, then `0` is returned.
/// If the byte index is past the end of the line, the column index of the last
/// character `+ 1` is returned.
///
/// # Example
///
/// ```rust
/// use codespan_reporting::files;
///
/// let line_start = 2;
/// let line_source = "🗻∈🌏";
///
/// assert_eq!(files::column_index(line_source, line_start, 0), 0);
/// assert_eq!(files::column_index(line_source, line_start, line_start + 0), 0);
/// assert_eq!(files::column_index(line_source, line_start, line_start + 1), 0);
/// assert_eq!(files::column_index(line_source, line_start, line_start + 4), 1);
/// assert_eq!(files::column_index(line_source, line_start, line_start + 8), 2);
/// assert_eq!(files::column_index(line_source, line_start, line_start + line_source.len()), 3);
/// ```
pub fn column_index(line_source: &str, line_start: usize, byte_index: usize) -> usize {
    match byte_index.checked_sub(line_start) {
        None => 0,
        Some(relative_index) => {
            let column_index = line_source
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|i| *i < relative_index)
                .count();

            match () {
                () if relative_index >= line_source.len() => column_index,
                () if line_source.is_char_boundary(relative_index) => column_index,
                () => column_index - 1,
            }
        }
    }
}

/// The 1-indexed column number at the given byte index.
///
/// # Example
///
/// ```rust
/// use codespan_reporting::files;
///
/// let line_start = 2;
/// let line_source = "🗻∈🌏";
///
/// assert_eq!(files::column_number(line_source, line_start, 0), 1);
/// assert_eq!(files::column_number(line_source, line_start, line_start + 0), 1);
/// assert_eq!(files::column_number(line_source, line_start, line_start + 1), 1);
/// assert_eq!(files::column_number(line_source, line_start, line_start + 4), 2);
/// assert_eq!(files::column_number(line_source, line_start, line_start + 8), 3);
/// assert_eq!(files::column_number(line_source, line_start, line_start + line_source.len()), 4);
/// ```
pub fn column_number(line_source: &str, line_start: usize, byte_index: usize) -> usize {
    column_index(line_source, line_start, byte_index) + 1
}

impl<Origin> SimpleFile<Origin>
where
    Origin: std::fmt::Display,
{
    /// Create a new source file.
    pub fn new(origin: Origin, source: impl Into<Arc<str>>) -> SimpleFile<Origin> {
        let source = source.into();
        SimpleFile {
            origin,
            line_starts: line_starts(&source).collect(),
            source,
        }
    }

    /// Return the origin of the file.
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// Return the source of the file.
    pub fn source(&self) -> &Arc<str> {
        &self.source
    }

    fn line_start(&self, line_index: usize) -> Option<usize> {
        use std::cmp::Ordering;

        match line_index.cmp(&self.line_starts.len()) {
            Ordering::Less => self.line_starts.get(line_index).cloned(),
            Ordering::Equal => Some(self.source.as_ref().len()),
            Ordering::Greater => None,
        }
    }

    fn line_range(&self, line_index: usize) -> Option<Range<usize>> {
        let line_start = self.line_start(line_index)?;
        let next_line_start = self.line_start(line_index + 1)?;

        Some(line_start..next_line_start)
    }
}

impl<Origin> Files for SimpleFile<Origin>
where
    Origin: std::fmt::Display + Clone,
{
    type FileId = ();
    type Origin = Origin;
    type Source = Arc<str>;

    fn origin(&self, (): ()) -> Option<Origin> {
        Some(self.origin.clone())
    }

    fn line_index(&self, (): (), byte_index: usize) -> Option<usize> {
        match self.line_starts.binary_search(&byte_index) {
            Ok(line) => Some(line),
            Err(next_line) => Some(next_line - 1),
        }
    }

    fn line(&self, (): (), line_index: usize) -> Option<Line> {
        Some(Line {
            range: self.line_range(line_index)?,
            number: line_index + 1,
        })
    }

    fn source(&self, (): ()) -> Option<Arc<str>> {
        Some(self.source.clone())
    }
}

/// A file database that can store multiple source files.
///
/// This is useful for simple language tests, but it might be worth creating a
/// custom implementation when a language scales beyond a certain size.
#[derive(Debug, Clone)]
pub struct SimpleFiles<Origin> {
    files: Vec<SimpleFile<Origin>>,
}

impl<Origin> SimpleFiles<Origin>
where
    Origin: std::fmt::Display,
{
    /// Create a new files database.
    pub fn new() -> SimpleFiles<Origin> {
        SimpleFiles { files: Vec::new() }
    }

    /// Add a file to the database, returning the handle that can be used to
    /// refer to it again.
    pub fn add(&mut self, origin: Origin, source: impl Into<Arc<str>>) -> usize {
        let file_id = self.files.len();
        self.files.push(SimpleFile::new(origin, source));
        file_id
    }

    /// Get the file corresponding to the given id.
    pub fn get(&self, file_id: usize) -> Option<&SimpleFile<Origin>> {
        self.files.get(file_id)
    }
}

impl<Origin> Files for SimpleFiles<Origin>
where
    Origin: std::fmt::Display + Clone,
{
    type FileId = usize;
    type Origin = Origin;
    type Source = Arc<str>;

    fn origin(&self, file_id: usize) -> Option<Origin> {
        Some(self.get(file_id)?.origin().clone())
    }

    fn line_index(&self, file_id: usize, byte_index: usize) -> Option<usize> {
        self.get(file_id)?.line_index((), byte_index)
    }

    fn line(&self, file_id: usize, line_index: usize) -> Option<Line> {
        self.get(file_id)?.line((), line_index)
    }

    fn source(&self, file_id: usize) -> Option<Arc<str>> {
        Some(self.get(file_id)?.source().clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const TEST_SOURCE: &str = "foo\nbar\r\n\nbaz";

    #[test]
    fn line_starts() {
        let file = SimpleFile::new("test", TEST_SOURCE);

        assert_eq!(
            file.line_starts,
            [
                0,  // "foo\n"
                4,  // "bar\r\n"
                9,  // ""
                10, // "baz"
            ],
        );
    }

    #[test]
    fn line_span_sources() {
        let file = SimpleFile::new("test", TEST_SOURCE);

        let line_sources = (0..4)
            .map(|line| {
                let line_range = file.line_range(line).unwrap();
                &file.source[line_range]
            })
            .collect::<Vec<_>>();

        assert_eq!(line_sources, ["foo\n", "bar\r\n", "\n", "baz"]);
    }
}
