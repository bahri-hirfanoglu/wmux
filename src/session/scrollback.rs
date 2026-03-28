/// Ring buffer for storing terminal output lines per pane.
///
/// Stores up to `capacity` lines. When full, the oldest line is evicted
/// on each new push (FIFO ring buffer behavior).
pub struct ScrollbackBuffer {
    lines: Vec<Vec<u8>>,
    capacity: usize,
    head: usize,
    count: usize,
    partial_line: Vec<u8>,
}

impl ScrollbackBuffer {
    /// Create a new empty scrollback buffer with the given line capacity.
    pub fn new(capacity: usize) -> Self {
        ScrollbackBuffer {
            lines: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            count: 0,
            partial_line: Vec::new(),
        }
    }

    /// Maximum number of lines this buffer can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current number of stored lines.
    pub fn line_count(&self) -> usize {
        self.count
    }

    /// Push a single line into the buffer.
    pub fn push_line(&mut self, line: Vec<u8>) {
        if self.lines.len() < self.capacity {
            // Buffer not yet full — append
            self.lines.push(line);
        } else {
            // Buffer full — overwrite at head position
            self.lines[self.head] = line;
        }
        self.head = (self.head + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    /// Push raw bytes, splitting on newline (0x0A) boundaries.
    ///
    /// Partial lines (data not ending with newline) are buffered internally
    /// and prepended to the next push_bytes call.
    pub fn push_bytes(&mut self, data: &[u8]) {
        let mut start = 0;
        for i in 0..data.len() {
            if data[i] == b'\n' {
                // Found a newline — complete the current line
                let mut line = std::mem::take(&mut self.partial_line);
                line.extend_from_slice(&data[start..i]);
                self.push_line(line);
                start = i + 1;
            }
        }
        // Buffer any remaining bytes as a partial line
        if start < data.len() {
            self.partial_line.extend_from_slice(&data[start..]);
        }
    }

    /// Get a line by logical index (0 = oldest stored line).
    pub fn get_line(&self, index: usize) -> Option<&[u8]> {
        if index >= self.count {
            return None;
        }
        let phys = self.physical_index(index);
        Some(&self.lines[phys])
    }

    /// Get a range of lines starting at `start` (logical index), up to `count` lines.
    ///
    /// Returns fewer lines if the range extends beyond stored lines.
    pub fn get_lines(&self, start: usize, count: usize) -> Vec<&[u8]> {
        let available = self.count.saturating_sub(start);
        let actual_count = count.min(available);

        let mut result = Vec::with_capacity(actual_count);
        for i in 0..actual_count {
            let phys = self.physical_index(start + i);
            result.push(self.lines[phys].as_slice());
        }
        result
    }

    /// Convert a logical index (0 = oldest) to the physical ring buffer index.
    fn physical_index(&self, logical_index: usize) -> usize {
        if self.count < self.capacity {
            // Buffer not yet wrapped — logical == physical
            logical_index
        } else {
            // Buffer has wrapped — oldest is at head (which is the next write position)
            (self.head + logical_index) % self.capacity
        }
    }
}
