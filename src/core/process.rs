/// Container for managing the process queue of warriors. A given core has
/// a single queue, but the queue itself may have numerous "threads" of execution
/// and determines what process is scheduled when.
use std::collections::{BTreeMap, VecDeque};

use thiserror::Error as ThisError;

use crate::core::WarriorID;

use crate::core::Offset;

#[derive(Debug, Eq, PartialEq)]
pub struct Entry {
    pub id: WarriorID,
    pub thread: usize,
    pub offset: Offset,
}

/// A representation of the process queue. This is effectively a simple FIFO queue.
// TODO enforce size limits based on MAXPROCESSES
#[derive(Debug)]
pub struct Queue {
    /// The actual offsets enqueued to be executed
    queue: VecDeque<Entry>,

    /// A map of process names to the number of tasks each has in the queue.
    /// This is updated whenever instructions are added to/removed from the queue,
    /// and can be used to determine whether a process is alive or not.
    processes: BTreeMap<WarriorID, usize>,

    /// An increasing counter per process to give unique thread ids
    next_thread_id: BTreeMap<WarriorID, usize>,
}

impl Queue {
    /// Create an empty queue
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            processes: BTreeMap::new(),
            next_thread_id: BTreeMap::new(),
        }
    }

    /// Get the next offset for execution, removing it from the queue.
    pub fn pop(&mut self) -> Result<Entry, Error> {
        self.queue
            .pop_front()
            .map_or(Err(Error::NoRemainingProcesses), |entry| {
                let decremented = self.processes[&entry.id].saturating_sub(1);
                self.processes
                    .entry(entry.id)
                    .and_modify(|count| *count = decremented);

                Ok(entry)
            })
    }

    /// Get the next offset for execution without modifying the queue.
    // TODO: this should probably just return Option<&ProcessEntry>
    pub fn peek(&self) -> Result<&Entry, Error> {
        self.queue.get(0).ok_or(Error::NoRemainingProcesses)
    }

    /// Add an entry to the process queue. If specified, it will use the given thread ID,
    /// otherwise a new thread ID will be created based on the current number of
    /// threads active for this process name.
    pub fn push(&mut self, warrior_id: WarriorID, offset: Offset, thread: Option<usize>) {
        let thread_id = thread.unwrap_or_else(|| {
            let entry = self.next_thread_id.entry(warrior_id).or_insert(0);
            let id = *entry;
            *entry += 1;
            id
        });

        self.queue.push_back(Entry {
            id: warrior_id,
            thread: thread_id,
            offset,
        });

        *self.processes.entry(warrior_id).or_insert(0) += 1;
    }

    /// Check the status of a process in the queue. Panics if the process was
    /// never added to the queue.
    pub fn thread_count(&self, warrrior_id: WarriorID) -> usize {
        self.processes[&warrrior_id]
    }
}

/// An process-related error occurred
#[derive(ThisError, Debug, Eq, PartialEq)]
pub enum Error {
    /// All processes terminated
    #[error("no process running to execute")]
    NoRemainingProcesses,

    /// The warrior attempted to execute a DAT instruction
    #[error("reached a DAT at offset {0}")]
    ExecuteDat(Offset),

    /// The warrior attempted to execute a division by zero
    #[error("division by 0")]
    DivideByZero,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_multiple_processes() {
        let mut queue = Queue::new();

        assert_eq!(queue.peek().unwrap_err(), Error::NoRemainingProcesses);
        assert_eq!(queue.pop().unwrap_err(), Error::NoRemainingProcesses);

        let starting_offset = Offset::new(10, 8000);

        queue.push(1, starting_offset, None);
        assert_eq!(
            queue.peek().unwrap(),
            &Entry {
                id: 1,
                thread: 0,
                offset: starting_offset
            }
        );
        assert!(queue.thread_count(1) > 0);

        queue.push(2, starting_offset + 5, None);
        assert!(queue.thread_count(2) > 0);

        assert_eq!(
            queue.pop().unwrap(),
            Entry {
                id: 1,
                thread: 0,
                offset: starting_offset
            }
        );
        assert_eq!(
            queue.peek().unwrap(),
            &Entry {
                id: 2,
                thread: 0,
                offset: starting_offset + 5
            }
        );
        assert!(!queue.thread_count(1) > 0);
        assert!(queue.thread_count(2) > 0);

        assert_eq!(
            queue.pop().unwrap(),
            Entry {
                id: 2,
                thread: 0,
                offset: starting_offset + 5
            }
        );
        assert!(!queue.thread_count(1) > 0);
        assert!(!queue.thread_count(2) > 0);

        assert_eq!(queue.peek().unwrap_err(), Error::NoRemainingProcesses);
        assert_eq!(queue.pop().unwrap_err(), Error::NoRemainingProcesses);

        assert!(!queue.thread_count(1) > 0);
        assert!(!queue.thread_count(2) > 0);
    }

    #[test]
    fn queue_single_process() {
        let mut queue = Queue::new();
        let starting_offset = Offset::new(10, 8000);

        queue.push(1, starting_offset, None);
        assert_eq!(
            queue.peek().unwrap(),
            &Entry {
                id: 1,
                thread: 0,
                offset: starting_offset
            }
        );
        assert!(queue.thread_count(1) > 0);

        // should increment the thread id to 1
        queue.push(1, starting_offset, None);
        queue.pop().unwrap();
        assert_eq!(
            queue.peek().unwrap(),
            &Entry {
                id: 1,
                thread: 1,
                offset: starting_offset
            }
        );
        assert!(queue.thread_count(1) > 0);

        queue.push(1, starting_offset, Some(1));
        queue.pop().unwrap();
        assert_eq!(
            queue.peek().unwrap(),
            &Entry {
                id: 1,
                thread: 1,
                offset: starting_offset
            }
        );
        assert!(queue.thread_count(1) > 0);
    }
}
