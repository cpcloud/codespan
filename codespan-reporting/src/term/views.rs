use std::io;
use std::ops::Range;
use termcolor::WriteColor;

use crate::diagnostic::{Diagnostic, LabelStyle};
use crate::files::{self, Files};
use crate::term::display_list::{Entry, Locus, Mark, MarkSeverity};
use crate::term::renderer::Renderer;
use crate::term::Config;

/// Count the number of decimal digits in `n`.
fn count_digits(mut n: usize) -> usize {
    let mut count = 0;
    while n != 0 {
        count += 1;
        n /= 10; // remove last digit
    }
    count
}

/// Output a richly formatted diagnostic, with source code previews.
pub struct RichDiagnostic<'a, FileId> {
    diagnostic: &'a Diagnostic<FileId>,
}

impl<'a, FileId> RichDiagnostic<'a, FileId>
where
    FileId: Copy + PartialEq,
{
    pub fn new(diagnostic: &'a Diagnostic<FileId>) -> RichDiagnostic<'a, FileId> {
        RichDiagnostic { diagnostic }
    }

    // TODO: Return display list, rather than rendering in place
    pub fn emit<'files>(
        &self,
        files: &'files impl Files<'files, FileId = FileId>,
        writer: &mut dyn WriteColor,
        config: &Config,
    ) -> io::Result<()>
    where
        FileId: 'files,
    {
        struct MarkedFile<'a, Origin, Source> {
            origin: Origin,
            marks: Vec<FileMark<'a, Source>>,
        }

        struct FileMark<'a, Source> {
            severity: MarkSeverity,
            lines: Vec<files::Line<Source>>,
            range: Range<usize>,
            message: &'a str,
        }

        // Group marks by file

        // TODO: Make this data structure external, to allow for allocation reuse
        let mut marked_files = Vec::new();
        let mut outer_padding = 0;

        for label in &self.diagnostic.labels {
            let severity = match label.style {
                LabelStyle::Primary => Some(self.diagnostic.severity),
                LabelStyle::Secondary => None,
            };

            let range = label.range.clone();
            let start_line_index = files
                .line_index(label.file_id, range.start)
                .expect("start_index");
            let end_line_index = files
                .line_index(label.file_id, range.end)
                .expect("end_index");

            let file_mark = FileMark {
                severity,
                range,
                lines: (start_line_index..=end_line_index)
                    .map(|line_index| {
                        let line = files.line(label.file_id, line_index).expect("line");
                        outer_padding = std::cmp::max(outer_padding, count_digits(line.number));
                        line
                    })
                    .collect(),
                message: label.message.as_str(),
            };

            // TODO: Group contiguous line index ranges using some sort of interval set algorithm.
            // TODO: Flatten mark groups to overlapping underlines that can be easily rendered.
            // TODO: If start line and end line are too far apart, we should add a source break.
            match marked_files
                .iter_mut()
                .find(|(file_id, _)| label.file_id == *file_id)
            {
                None => marked_files.push((
                    label.file_id,
                    MarkedFile {
                        origin: files.origin(label.file_id).expect("origin"),
                        marks: vec![file_mark],
                    },
                )),
                Some((_, marked_file)) => {
                    marked_file.marks.push(file_mark);
                }
            }
        }

        // Sort marks lexicographically by the range of source code they cover.
        for (_, marked_file) in marked_files.iter_mut() {
            marked_file.marks.sort_by_key(|mark| {
                // `Range<usize>` doesn't implement `Ord`, so convert to `(usize, usize)`
                // to piggyback off its lexicographic sorting implementation.
                (mark.range.start, mark.range.end)
            });
        }

        let mut renderer = Renderer::new(writer, config);

        // Header and message
        //
        // ```text
        // error[E0001]: unexpected type in `+` application
        // ```
        renderer.render(&Entry::Header {
            locus: None,
            severity: self.diagnostic.severity,
            code: self.diagnostic.code.as_ref().map(String::as_str),
            message: self.diagnostic.message.as_str(),
        })?;
        if !marked_files.is_empty() {
            renderer.render(&Entry::Empty)?;
        }

        // Source snippets
        //
        // ```text
        //   ┌── test:2:9 ───
        //   │
        // 2 │ (+ test "")
        //   │         ^^ expected `Int` but found `String`
        //   │
        // ```
        for (_, marked_file) in &marked_files {
            // Top left border and locus.
            //
            // ```text
            // ┌── test:2:9 ───
            // ```

            // Fixup location
            let locus = match marked_file.marks.first() {
                None => continue,
                Some(first_mark) => {
                    let first_line = first_mark.lines.first().unwrap();
                    Locus {
                        origin: marked_file.origin.to_string(),
                        line_number: first_line.number,
                        column_number: first_line.column_number(first_mark.range.start),
                    }
                }
            };

            renderer.render(&Entry::SourceStart {
                outer_padding,
                locus,
            })?;

            // Code snippet
            //
            // ```text
            //   │
            // 2 │ (+ test "")
            //   │         ^^ expected `Int` but found `String`
            //   │
            // ```
            for (i, file_mark) in marked_file.marks.iter().enumerate() {
                match i {
                    0 => renderer.render(&Entry::SourceEmpty {
                        outer_padding,
                        left_marks: Vec::new(),
                    })?,
                    _ => renderer.render(&Entry::SourceBreak {
                        outer_padding,
                        left_marks: Vec::new(),
                    })?,
                };

                // Attempt to split off the first line.
                let (start_line, remaining_lines) = match file_mark.lines.split_first() {
                    // No lines! This is probably a bug...
                    None => continue,
                    // At least one line...
                    Some(split) => split,
                };

                // Attempt to split off the last line.
                match remaining_lines.split_last() {
                    // Single line
                    //
                    // ```text
                    // 2 │ (+ test "")
                    //   │         ^^ expected `Int` but found `String`
                    // ```
                    None => {
                        let mark_start = file_mark.range.start - start_line.start;
                        let mark_end = file_mark.range.end - start_line.start;

                        renderer.render(&Entry::SourceLine {
                            outer_padding,
                            line_number: start_line.number,
                            source: start_line.source.as_ref(),
                            marks: vec![Some((
                                file_mark.severity,
                                Mark::Single(mark_start..mark_end, &file_mark.message),
                            ))],
                        })?;
                    }
                    // Multiple lines
                    //
                    // ```text
                    // 4 │   fizz₁ num = case (mod num 5) (mod num 3) of
                    //   │ ╭─────────────^
                    // 5 │ │     0 0 => "FizzBuzz"
                    // 6 │ │     0 _ => "Fizz"
                    // 7 │ │     _ 0 => "Buzz"
                    // 8 │ │     _ _ => num
                    //   │ ╰──────────────^ `case` clauses have incompatible types
                    // ```
                    Some((end_line, marked_lines)) => {
                        let start_source = start_line.source.as_ref();
                        let end_source = end_line.source.as_ref();

                        let mark_start = file_mark.range.start - start_line.start;
                        let prefix_source = &start_source[..mark_start];

                        if prefix_source.trim().is_empty() {
                            // Section is prefixed by empty space, so we don't need to take
                            // up a new line.
                            //
                            // ```text
                            // 4 │ ╭     case (mod num 5) (mod num 3) of
                            // ```
                            renderer.render(&Entry::SourceLine {
                                outer_padding,
                                line_number: start_line.number,
                                source: &start_source,
                                marks: vec![Some((file_mark.severity, Mark::MultiTopLeft))],
                            })?;
                        } else {
                            // There's source code in the prefix, so run an underline
                            // underneath it to get to the start of the range.
                            //
                            // ```text
                            // 4 │   fizz₁ num = case (mod num 5) (mod num 3) of
                            //   │ ╭─────────────^
                            // ```
                            renderer.render(&Entry::SourceLine {
                                outer_padding,
                                line_number: start_line.number,
                                source: &start_source,
                                marks: vec![Some((
                                    file_mark.severity,
                                    Mark::MultiTop(..mark_start),
                                ))],
                            })?;
                        }

                        // Write marked lines
                        //
                        // ```text
                        // 5 │ │     0 0 => "FizzBuzz"
                        // 6 │ │     0 _ => "Fizz"
                        // 7 │ │     _ 0 => "Buzz"
                        // ```
                        for marked_line in marked_lines {
                            renderer.render(&Entry::SourceLine {
                                outer_padding,
                                line_number: marked_line.number,
                                source: marked_line.source.as_ref(),
                                marks: vec![Some((file_mark.severity, Mark::MultiLeft))],
                            })?;
                        }

                        // Write last marked line
                        //
                        // ```text
                        // 8 │ │     _ _ => num
                        //   │ ╰──────────────^ `case` clauses have incompatible types
                        // ```
                        let mark_end = file_mark.range.end - end_line.start;

                        renderer.render(&Entry::SourceLine {
                            outer_padding,
                            line_number: end_line.number,
                            source: end_source,
                            marks: vec![Some((
                                file_mark.severity,
                                Mark::MultiBottom(..mark_end, &file_mark.message),
                            ))],
                        })?;
                    }
                }
            }
            renderer.render(&Entry::SourceEmpty {
                outer_padding,
                left_marks: Vec::new(),
            })?;
        }

        // Additional notes
        //
        // ```text
        // = expected type `Int`
        //      found type `String`
        // ```
        for note in &self.diagnostic.notes {
            renderer.render(&Entry::SourceNote {
                outer_padding,
                message: note,
            })?;
        }
        renderer.render(&Entry::Empty)?;

        Ok(())
    }
}

/// Output a short diagnostic, with a line number, severity, and message.
pub struct ShortDiagnostic<'a, FileId> {
    diagnostic: &'a Diagnostic<FileId>,
}

impl<'a, FileId> ShortDiagnostic<'a, FileId>
where
    FileId: Copy + PartialEq,
{
    pub fn new(diagnostic: &'a Diagnostic<FileId>) -> ShortDiagnostic<'a, FileId> {
        ShortDiagnostic { diagnostic }
    }

    // TODO: Return display list, rather than rendering in place
    pub fn emit<'files>(
        &self,
        files: &'files impl Files<'files, FileId = FileId>,
        writer: &mut dyn WriteColor,
        config: &Config,
    ) -> io::Result<()>
    where
        FileId: 'files,
    {
        let mut renderer = Renderer::new(writer, config);

        // Located headers
        //
        // ```text
        // test:2:9: error[E0001]: unexpected type in `+` application
        // ```
        let mut primary_labels_encountered = 0;
        let labels = self.diagnostic.labels.iter();
        for label in labels.filter(|label| label.style == LabelStyle::Primary) {
            primary_labels_encountered += 1;

            let origin = files.origin(label.file_id).expect("origin");
            let start = label.range.start;
            let line_index = files.line_index(label.file_id, start).expect("line_index");
            let line = files.line(label.file_id, line_index).expect("line");

            renderer.render(&Entry::Header {
                locus: Some(Locus {
                    origin: origin.to_string(),
                    line_number: line.number,
                    column_number: line.column_number(start),
                }),
                severity: self.diagnostic.severity,
                code: self.diagnostic.code.as_ref().map(String::as_str),
                message: self.diagnostic.message.as_str(),
            })?;
        }

        // Fallback to printing a non-located header if no primary labels were encountered
        //
        // ```text
        // error[E0002]: Bad config found
        // ```
        if primary_labels_encountered == 0 {
            renderer.render(&Entry::Header {
                locus: None,
                severity: self.diagnostic.severity,
                code: self.diagnostic.code.as_ref().map(String::as_str),
                message: self.diagnostic.message.as_str(),
            })?;
        }

        Ok(())
    }
}
