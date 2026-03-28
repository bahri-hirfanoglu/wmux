use wmux::session::scrollback::ScrollbackBuffer;

#[test]
fn new_buffer_has_zero_lines_and_correct_capacity() {
    let buf = ScrollbackBuffer::new(10_000);
    assert_eq!(buf.line_count(), 0);
    assert_eq!(buf.capacity(), 10_000);
}

#[test]
fn push_single_line_and_retrieve() {
    let mut buf = ScrollbackBuffer::new(10_000);
    buf.push_line(b"hello world".to_vec());
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.get_line(0), Some(b"hello world".as_slice()));
}

#[test]
fn push_over_capacity_evicts_oldest() {
    let mut buf = ScrollbackBuffer::new(100);
    for i in 0..101 {
        buf.push_line(format!("line {}", i).into_bytes());
    }
    assert_eq!(buf.line_count(), 100);
    // Oldest line (line 0) should be evicted; line 1 is now the oldest
    assert_eq!(buf.get_line(0), Some(b"line 1".as_slice()));
    // Last line is line 100
    assert_eq!(buf.get_line(99), Some(b"line 100".as_slice()));
}

#[test]
fn get_lines_returns_correct_slice_with_wraparound() {
    let mut buf = ScrollbackBuffer::new(5);
    // Push 7 lines into a capacity-5 buffer to force wraparound
    for i in 0..7 {
        buf.push_line(format!("line {}", i).into_bytes());
    }
    assert_eq!(buf.line_count(), 5);
    // Buffer should contain lines 2..6
    let lines = buf.get_lines(0, 5);
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], b"line 2");
    assert_eq!(lines[1], b"line 3");
    assert_eq!(lines[2], b"line 4");
    assert_eq!(lines[3], b"line 5");
    assert_eq!(lines[4], b"line 6");

    // Partial range
    let partial = buf.get_lines(1, 3);
    assert_eq!(partial.len(), 3);
    assert_eq!(partial[0], b"line 3");
    assert_eq!(partial[1], b"line 4");
    assert_eq!(partial[2], b"line 5");
}

#[test]
fn push_bytes_splits_on_newlines() {
    let mut buf = ScrollbackBuffer::new(100);
    buf.push_bytes(b"first line\nsecond line\nthird line\n");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.get_line(0), Some(b"first line".as_slice()));
    assert_eq!(buf.get_line(1), Some(b"second line".as_slice()));
    assert_eq!(buf.get_line(2), Some(b"third line".as_slice()));
}

#[test]
fn push_bytes_handles_partial_lines() {
    let mut buf = ScrollbackBuffer::new(100);
    // Push data without trailing newline — partial line buffered
    buf.push_bytes(b"partial");
    assert_eq!(buf.line_count(), 0);

    // Complete the line
    buf.push_bytes(b" line\n");
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.get_line(0), Some(b"partial line".as_slice()));
}

#[test]
fn long_line_stored_as_is() {
    let mut buf = ScrollbackBuffer::new(100);
    let long_line = "x".repeat(500);
    buf.push_line(long_line.as_bytes().to_vec());
    assert_eq!(buf.line_count(), 1);
    assert_eq!(buf.get_line(0).unwrap().len(), 500);
}

#[test]
fn get_line_out_of_bounds_returns_none() {
    let buf = ScrollbackBuffer::new(100);
    assert_eq!(buf.get_line(0), None);

    let mut buf2 = ScrollbackBuffer::new(100);
    buf2.push_line(b"only".to_vec());
    assert_eq!(buf2.get_line(1), None);
}

#[test]
fn get_lines_clamped_to_available() {
    let mut buf = ScrollbackBuffer::new(100);
    buf.push_line(b"one".to_vec());
    buf.push_line(b"two".to_vec());
    // Request more lines than available
    let lines = buf.get_lines(0, 10);
    assert_eq!(lines.len(), 2);
}
