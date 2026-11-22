//! Cooperative Task Switching & Context Management for JainOS.
//!
//! This module implements genuine x86_64 low-level cooperative context switching
//! adhering to the System V AMD64 ABI.
//!
//! It provides:
//! - `switch_context(old_rsp: *mut usize, new_rsp: usize)` implemented in assembly.
//! - Preserves callee-saved registers (RBP, RBX, R12..R15) and stack pointers (RSP).
//! - Mathematically verified 16-byte stack alignment.
//! - Verification test demonstrating independent stacks and state retention.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::arch::global_asm;
use crate::log_info;
use crate::sync::Spinlock;

// Global assembly implementation of switch_context
global_asm!(
    r#"
    .global switch_context
    .type switch_context, @function
    switch_context:
        # 1. Disable interrupts during stack pointer transition
        cli

        # 2. Save outgoing task's sanitized RFLAGS (passed in RDX)
        push rdx

        # 3. Save callee-saved registers (R15..RBP) in exact order
        push r15
        push r14
        push r13
        push r12
        push rbx
        push rbp

        # 4. Save outgoing task's RSP through old_rsp (RDI contains old_rsp)
        mov [rdi], rsp

        # 5. Load incoming task's RSP (RSI contains new_rsp)
        mov rsp, rsi

        # 6. Restore incoming task's callee-saved registers in reverse order
        pop rbp
        pop rbx
        pop r12
        pop r13
        pop r14
        pop r15

        # 7. Restore incoming task's sanitized flags (restores IF state)
        popfq

        # 8. Return into incoming task's saved RIP
        ret
    "#
);

extern "C" {
    /// Low-level assembly routine performing a cooperative context switch.
    ///
    /// # Parameters:
    /// - `old_rsp`: Pointer to storage for outgoing task's saved RSP (`RDI`).
    /// - `new_rsp`: Incoming task's saved RSP (`RSI`).
    /// - `sanitized_rflags`: Sanitized CPU flags (IF and reserved bit) for outgoing task (`RDX`).
    ///
    /// # Safety
    /// - `old_rsp` must be a valid, writable pointer to store the current stack pointer.
    /// - `new_rsp` must be a valid stack pointer for an initialized or suspended task stack.
    pub fn switch_context(old_rsp: *mut usize, new_rsp: usize, sanitized_rflags: usize);
}

/// Stack size for cooperative test tasks (16 KiB = 4 pages).
pub const TASK_STACK_SIZE: usize = 4096 * 4;

/// Dedicated stack storage aligned to 16 bytes.
///
/// # Stack Ownership Rules:
/// 1. **Dynamic Stacks**: Created via `TaskStack::new_dynamic()` on the kernel heap.
///    Ownership is moved into a `TaskControlBlock` via `Box<TaskStack>`. When the TCB
///    is dropped during task reclamation, Rust automatically frees the stack via `ALLOCATOR.dealloc()`.
/// 2. **Static / Boot Stacks**: Statically allocated or provided by the bootloader.
///    The `TaskControlBlock` holds `stack: None`, preventing inadvertent heap deallocation.
/// 3. **Active Execution Invariant**: A task's stack memory must never be freed while
///    the task is in `TaskState::Running` or during `switch_context`. Reclamation strictly
///    occurs only after a task transitions to `TaskState::Terminated` and has been switched away.
#[repr(align(16))]
pub struct TaskStack(pub [u8; TASK_STACK_SIZE]);

impl TaskStack {
    /// Allocates a new zero-initialized 16 KiB task stack directly on the kernel heap.
    ///
    /// Uses `alloc_zeroed` to prevent allocating a large 16 KiB temporary array on the current stack.
    /// Returns `None` if kernel heap memory is exhausted.
    pub fn new_dynamic() -> Option<Box<Self>> {
        let layout = Layout::new::<TaskStack>();
        unsafe {
            let ptr = alloc::alloc::alloc_zeroed(layout) as *mut TaskStack;
            if ptr.is_null() {
                None
            } else {
                Some(Box::from_raw(ptr))
            }
        }
    }

    /// Returns the raw memory address of the bottom (lowest address) of the stack.
    #[inline(always)]
    pub fn bottom(&self) -> usize {
        self.0.as_ptr() as usize
    }

    /// Returns the 16-byte aligned memory address of the top (highest address) of the stack.
    #[inline(always)]
    pub fn top(&self) -> usize {
        let bottom = self.bottom();
        (bottom + TASK_STACK_SIZE) & !0xF
    }
}

impl core::fmt::Debug for TaskStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TaskStack(bottom: {:#x}, top: {:#x}, size: {} KiB)",
            self.bottom(),
            self.top(),
            TASK_STACK_SIZE / 1024
        )
    }
}

/// Unique identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(pub usize);

impl TaskId {
    /// Creates a new TaskId from a raw integer.
    pub const fn new(id: usize) -> Self {
        TaskId(id)
    }

    /// Returns the raw usize value of the TaskId.
    pub const fn as_usize(&self) -> usize {
        self.0
    }

    /// Returns the raw u64 value of the TaskId.
    pub const fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

impl core::fmt::Display for TaskId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Task({})", self.0)
    }
}

/// Represents the lifecycle state of a task in JainOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Task is ready to be scheduled and executed.
    Ready,
    /// Task is currently executing on the CPU.
    Running,
    /// Task is blocked waiting for an event, timer, or I/O.
    Blocked,
    /// Task has completed execution and is awaiting reclamation.
    Terminated,
}

/// Error returned when attempting an illegal task lifecycle transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTransitionError {
    /// A Terminated task cannot transition to any other state (terminal state).
    AlreadyTerminated,
    /// The requested transition is architecturally invalid.
    InvalidTransition {
        from: TaskState,
        to: TaskState,
    },
    /// A task cannot transition to its own identical state redundantly.
    RedundantTransition,
}

/// RFLAGS Bit 9: Interrupt Enable Flag (IF).
pub const RFLAGS_IF_MASK: usize = 1 << 9;

/// RFLAGS Bit 1: Mandatory reserved bit that must always be 1 in RFLAGS.
pub const RFLAGS_RESERVED: usize = 1 << 1;

/// Initial canonical sanitized RFLAGS for newly spawned tasks (IF=1, Reserved=1).
pub const INITIAL_RFLAGS_SANITIZED: usize = RFLAGS_IF_MASK | RFLAGS_RESERVED; // 0x0202

/// Task Control Block (TCB) representing an execution context in JainOS.
///
/// Contains all metadata, execution statistics, saved register state,
/// and owned resources for a task.
pub struct TaskControlBlock {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable task name (e.g., "Main", "Idle", "Alpha").
    pub name: &'static str,
    /// Current lifecycle state (private: mutations must go through transition_to).
    state: TaskState,
    /// Saved stack pointer (RSP) pointing to the top of the suspended stack frame.
    pub saved_rsp: usize,
    /// Lower memory address of the stack boundary.
    pub stack_bottom: usize,
    /// Upper 16-byte aligned memory address of the stack boundary.
    pub stack_top: usize,
    /// Task entry function pointer (None for Main/Boot context).
    pub entry_fn: Option<extern "C" fn()>,
    /// Execution statistics: Number of times this task was scheduled.
    pub switch_count: u64,
    /// Execution statistics: Total timer ticks consumed while running.
    pub total_ticks: u64,
    /// Execution statistics: Timestamp of the tick when last scheduled.
    pub last_scheduled_tick: u64,
    /// Owned stack memory on the heap (None for static/bootloader stacks).
    pub stack: Option<Box<TaskStack>>,
}

impl TaskControlBlock {
    /// Returns the current lifecycle state of the task (read-only).
    #[inline(always)]
    pub fn state(&self) -> TaskState {
        self.state
    }
    /// Creates a new TCB for a dynamically allocated task.
    pub fn new_dynamic(
        id: TaskId,
        name: &'static str,
        entry_fn: extern "C" fn(),
        stack: Box<TaskStack>,
        initial_rsp: usize,
    ) -> Self {
        let stack_bottom = stack.bottom();
        let stack_top = stack.top();
        Self {
            id,
            name,
            state: TaskState::Ready,
            saved_rsp: initial_rsp,
            stack_bottom,
            stack_top,
            entry_fn: Some(entry_fn),
            switch_count: 0,
            total_ticks: 0,
            last_scheduled_tick: 0,
            stack: Some(stack),
        }
    }

    /// Creates a TCB representing the initial Main/Boot kernel context.
    pub fn new_main(id: TaskId, name: &'static str, current_rsp: usize) -> Self {
        Self {
            id,
            name,
            state: TaskState::Running,
            saved_rsp: current_rsp,
            stack_bottom: 0,
            stack_top: 0,
            entry_fn: None,
            switch_count: 1,
            total_ticks: 0,
            last_scheduled_tick: 0,
            stack: None,
        }
    }

    /// Verifies whether a given stack pointer falls strictly within the allocated stack boundaries.
    #[inline(always)]
    pub fn is_stack_within_bounds(&self, ptr: usize) -> bool {
        if self.stack_bottom == 0 && self.stack_top == 0 {
            // Main/boot stack without known static boundaries
            true
        } else {
            ptr >= self.stack_bottom && ptr <= self.stack_top
        }
    }

    /// Records a context switch event and updates execution statistics.
    #[inline(always)]
    pub fn record_context_switch(&mut self, current_tick: u64) {
        self.switch_count = self.switch_count.saturating_add(1);
        self.last_scheduled_tick = current_tick;
    }

    /// Transitions the task to a new state with strict lifecycle validation.
    ///
    /// # Permitted Transitions:
    /// - `Ready -> Running` (scheduled by CPU)
    /// - `Running -> Ready` (cooperative yield)
    /// - `Running -> Blocked` (waiting on I/O, timer, or event)
    /// - `Running -> Terminated` (task completed execution)
    /// - `Blocked -> Ready` (event fired, task unblocked)
    /// - `Blocked -> Terminated` (cancelled while blocked)
    /// - `Ready -> Terminated` (cancelled before running)
    ///
    /// # Forbidden Transitions:
    /// - `Terminated -> *`: Once terminated, a task can NEVER transition back.
    /// - `Blocked -> Running`: Must transition through Ready to be scheduled.
    /// - `Ready -> Blocked`: Must run first before blocking.
    /// - `State -> State`: Redundant transitions are rejected.
    pub fn transition_to(&mut self, new_state: TaskState) -> Result<(), StateTransitionError> {
        if self.state == TaskState::Terminated {
            return Err(StateTransitionError::AlreadyTerminated);
        }
        if self.state == new_state {
            return Err(StateTransitionError::RedundantTransition);
        }

        let is_valid = match (self.state, new_state) {
            (TaskState::Ready, TaskState::Running) => true,
            (TaskState::Ready, TaskState::Terminated) => true,
            (TaskState::Running, TaskState::Ready) => true,
            (TaskState::Running, TaskState::Blocked) => true,
            (TaskState::Running, TaskState::Terminated) => true,
            (TaskState::Blocked, TaskState::Ready) => true,
            (TaskState::Blocked, TaskState::Terminated) => true,
            _ => false,
        };

        if is_valid {
            self.state = new_state;
            Ok(())
        } else {
            Err(StateTransitionError::InvalidTransition {
                from: self.state,
                to: new_state,
            })
        }
    }

    /// Updates the task's lifecycle state with full validation.
    #[inline(always)]
    pub fn set_state(&mut self, new_state: TaskState) -> Result<(), StateTransitionError> {
        self.transition_to(new_state)
    }
}

impl core::fmt::Debug for TaskControlBlock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TaskControlBlock")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("state", &self.state)
            .field("saved_rsp", &format_args!("{:#x}", self.saved_rsp))
            .field("stack_bottom", &format_args!("{:#x}", self.stack_bottom))
            .field("stack_top", &format_args!("{:#x}", self.stack_top))
            .field("switch_count", &self.switch_count)
            .field("total_ticks", &self.total_ticks)
            .field("has_owned_stack", &self.stack.is_some())
            .finish()
    }
}

/// Maximum number of concurrent tasks supported by the scheduler.
pub const MAX_TASKS: usize = 32;

/// Error returned by ReadyQueue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// The queue has reached maximum capacity (MAX_TASKS).
    Full,
    /// The queue is empty.
    Empty,
    /// The specified TaskId is already present in the ready queue.
    DuplicateEntry(TaskId),
}

/// A fixed-capacity FIFO ring buffer storing TaskId elements.
///
/// Preallocated inline without dynamic heap memory allocation.
/// Guarantees that queue operations never invoke the kernel memory allocator.
pub struct ReadyQueue {
    buffer: [TaskId; MAX_TASKS],
    head: usize,
    tail: usize,
    count: usize,
}

impl ReadyQueue {
    /// Creates a new, empty ready queue.
    pub const fn new() -> Self {
        Self {
            buffer: [TaskId(0); MAX_TASKS],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Returns `true` if the ready queue contains no tasks.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns `true` if the ready queue has reached its maximum capacity.
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.count == MAX_TASKS
    }

    /// Returns the number of tasks currently waiting in the ready queue.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns the maximum capacity of the queue (MAX_TASKS).
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        MAX_TASKS
    }

    /// Checks if a task is already present in the queue.
    pub fn contains(&self, id: TaskId) -> bool {
        for i in 0..self.count {
            let idx = (self.head + i) % MAX_TASKS;
            if self.buffer[idx] == id {
                return true;
            }
        }
        false
    }

    /// Appends a task to the back of the ready queue.
    ///
    /// # Errors
    /// - Returns `Err(QueueError::Full)` if the queue is full.
    /// - Returns `Err(QueueError::DuplicateEntry(id))` if the task is already queued.
    pub fn push_back(&mut self, id: TaskId) -> Result<(), QueueError> {
        if self.is_full() {
            return Err(QueueError::Full);
        }
        if self.contains(id) {
            return Err(QueueError::DuplicateEntry(id));
        }
        self.buffer[self.tail] = id;
        self.tail = (self.tail + 1) % MAX_TASKS;
        self.count += 1;
        Ok(())
    }

    /// Removes and returns the task at the front of the ready queue.
    ///
    /// # Errors
    /// - Returns `Err(QueueError::Empty)` if the queue is empty.
    pub fn pop_front(&mut self) -> Result<TaskId, QueueError> {
        if self.is_empty() {
            return Err(QueueError::Empty);
        }
        let id = self.buffer[self.head];
        self.head = (self.head + 1) % MAX_TASKS;
        self.count -= 1;
        Ok(id)
    }

    /// Returns the task at the front of the ready queue without removing it.
    ///
    /// # Errors
    /// - Returns `Err(QueueError::Empty)` if the queue is empty.
    pub fn peek_front(&self) -> Result<TaskId, QueueError> {
        if self.is_empty() {
            return Err(QueueError::Empty);
        }
        Ok(self.buffer[self.head])
    }

    /// Clears all tasks from the ready queue.
    pub fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.count = 0;
    }
}

/// Error conditions that can occur during Scheduler operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    /// The task table has reached maximum capacity (MAX_TASKS = 32).
    TaskTableFull,
    /// The specified TaskId does not exist in the task table.
    TaskNotFound(TaskId),
    /// A task with this TaskId already exists in the task table.
    DuplicateTaskId(TaskId),
    /// An idle task has already been registered.
    IdleTaskAlreadyRegistered,
    /// Cannot remove or terminate the dedicated idle task.
    CannotTerminateIdleTask,
    /// Cannot remove or free the currently executing task.
    CannotRemoveCurrentTask,
    /// The task table slot is already occupied.
    SlotOccupied(usize),
    /// The task table slot was not reserved.
    SlotNotReserved(usize),
    /// The slot index is out of bounds.
    InvalidSlot(usize),
    /// An operation on the ready queue failed.
    QueueError(QueueError),
    /// A task lifecycle state transition was rejected.
    StateTransition(StateTransitionError),
    /// The task is not in the required Ready state for commit.
    InvalidTaskState(TaskState),
}

impl From<QueueError> for SchedulerError {
    fn from(err: QueueError) -> Self {
        SchedulerError::QueueError(err)
    }
}

impl From<StateTransitionError> for SchedulerError {
    fn from(err: StateTransitionError) -> Self {
        SchedulerError::StateTransition(err)
    }
}

/// Errors that can occur during dynamic task creation (spawn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The task table is full (MAX_TASKS = 32 reached).
    TaskTableFull,
    /// The ready queue has reached maximum capacity.
    ReadyQueueFull,
    /// Memory allocation failed (kernel heap exhausted).
    OutOfMemory,
    /// An error occurred within the scheduler.
    Scheduler(SchedulerError),
}

impl From<SchedulerError> for SpawnError {
    fn from(err: SchedulerError) -> Self {
        SpawnError::Scheduler(err)
    }
}

const EMPTY_TCB_SLOT: Option<TaskControlBlock> = None;

/// Fixed-capacity cooperative scheduler for JainOS.
///
/// Preallocated to guarantee that scheduler operations never invoke
/// the kernel heap allocator while synchronization locks are held.
pub struct Scheduler {
    /// Fixed-capacity task table storing active and suspended tasks.
    tasks: [Option<TaskControlBlock>; MAX_TASKS],
    /// Reservation bitmap tracking slots held during multi-phase dynamic creation.
    reserved_slots: [bool; MAX_TASKS],
    /// Fixed-capacity FIFO queue of runnable task identifiers.
    ready_queue: ReadyQueue,
    /// Task identifier of the currently executing task (None during early boot).
    current_task_id: Option<TaskId>,
    /// Dedicated TaskId of the system Idle Task.
    idle_task_id: Option<TaskId>,
    /// Monotonically increasing task ID counter to guarantee unique task identity
    /// independent of physical table slot indexes (avoids ABA recycling issues).
    next_task_id: usize,
}

impl Scheduler {
    /// Creates a new, uninitialized Scheduler with empty storage.
    pub const fn new() -> Self {
        Self {
            tasks: [EMPTY_TCB_SLOT; MAX_TASKS],
            reserved_slots: [false; MAX_TASKS],
            ready_queue: ReadyQueue::new(),
            current_task_id: None,
            idle_task_id: None,
            next_task_id: 1, // TaskId(0) can be reserved for Main/Boot context
        }
    }

    /// Allocates the next unique, monotonic TaskId.
    pub fn allocate_task_id(&mut self) -> TaskId {
        let id = TaskId(self.next_task_id);
        self.next_task_id = self.next_task_id.checked_add(1).expect("TaskId counter overflow");
        id
    }

    /// Iterates over all active tasks, invoking the closure on each TCB.
    pub fn for_each_task<F: FnMut(&TaskControlBlock)>(&self, mut f: F) {
        for slot in self.tasks.iter().flatten() {
            f(slot);
        }
    }

    /// Returns the currently executing task ID, if any.
    #[inline(always)]
    pub fn current_task_id(&self) -> Option<TaskId> {
        self.current_task_id
    }

    /// Sets the currently executing task ID.
    #[inline(always)]
    pub fn set_current_task_id(&mut self, id: Option<TaskId>) {
        self.current_task_id = id;
    }

    /// Returns the dedicated Idle Task ID, if registered.
    #[inline(always)]
    pub fn idle_task_id(&self) -> Option<TaskId> {
        self.idle_task_id
    }

    /// Registers the dedicated Idle Task.
    ///
    /// Validates that the task exists in the task table and rejects multiple registrations.
    pub fn set_idle_task(&mut self, id: TaskId) -> Result<(), SchedulerError> {
        if self.idle_task_id.is_some() {
            return Err(SchedulerError::IdleTaskAlreadyRegistered);
        }
        if self.find_task_slot(id).is_none() {
            return Err(SchedulerError::TaskNotFound(id));
        }
        self.idle_task_id = Some(id);
        Ok(())
    }

    /// Returns `true` if the given TaskId is the permanently reserved Idle Task.
    #[inline(always)]
    pub fn is_idle_task(&self, id: TaskId) -> bool {
        self.idle_task_id == Some(id)
    }

    /// Finds the table slot index containing the specified task.
    pub fn find_task_slot(&self, id: TaskId) -> Option<usize> {
        for (slot, entry) in self.tasks.iter().enumerate() {
            if let Some(ref tcb) = entry {
                if tcb.id == id {
                    return Some(slot);
                }
            }
        }
        None
    }

    /// Finds the first unoccupied, unreserved slot in the task table.
    ///
    /// Returns `Some(slot_index)` if capacity is available, or `None` if the table is full.
    pub fn find_free_slot(&self) -> Option<usize> {
        for slot in 0..MAX_TASKS {
            if self.tasks[slot].is_none() && !self.reserved_slots[slot] {
                return Some(slot);
            }
        }
        None
    }

    /// Safely reserves a free slot and allocates a monotonic TaskId.
    ///
    /// Pre-validates that the ready queue also has capacity.
    pub fn reserve_slot(&mut self) -> Result<(usize, TaskId), SpawnError> {
        if self.ready_queue.is_full() {
            return Err(SpawnError::ReadyQueueFull);
        }
        let slot = self.find_free_slot().ok_or(SpawnError::TaskTableFull)?;
        self.reserved_slots[slot] = true;
        let id = self.allocate_task_id();
        Ok((slot, id))
    }

    /// Releases a reserved slot in case task creation fails.
    pub fn release_reservation(&mut self, slot: usize) {
        if slot < MAX_TASKS {
            self.reserved_slots[slot] = false;
        }
    }

    /// Commits a fully initialized spawned task into a reserved slot and enqueues it.
    ///
    /// # Atomic Commit Invariants:
    /// 1. Validates slot bounds, slot reservation, and slot vacancy.
    /// 2. Validates that `tcb.id` is not already present in the task table.
    /// 3. Validates that the ready queue has capacity (`!ready_queue.is_full()`).
    /// 4. Ensures the task is in `TaskState::Ready`.
    /// 5. Performs ready queue insertion FIRST. If queue insertion fails, the task table
    ///    is completely unmodified, the slot remains reserved, and the uncommitted TCB
    ///    is returned to the caller.
    /// 6. Stores the TCB into `self.tasks[slot]`.
    /// 7. Clears `self.reserved_slots[slot]` ONLY after all fallible steps succeed.
    /// 8. On failure, returns `(SchedulerError, TaskControlBlock)` so the caller can
    ///    safely deallocate the heap stack *outside* the scheduler lock.
    pub fn commit_spawned_task(
        &mut self,
        slot: usize,
        tcb: TaskControlBlock,
    ) -> Result<(), (SchedulerError, TaskControlBlock)> {
        // 1. Validate slot bounds
        if slot >= MAX_TASKS {
            return Err((SchedulerError::InvalidSlot(slot), tcb));
        }

        // 2. Validate slot reservation
        if !self.reserved_slots[slot] {
            return Err((SchedulerError::SlotNotReserved(slot), tcb));
        }

        // 3. Validate slot vacancy
        if self.tasks[slot].is_some() {
            return Err((SchedulerError::SlotOccupied(slot), tcb));
        }

        // 4. Validate that TaskId is not already present in the task table
        if self.find_task_slot(tcb.id).is_some() {
            return Err((SchedulerError::DuplicateTaskId(tcb.id), tcb));
        }

        // 5. Validate that the ready queue has capacity
        if self.ready_queue.is_full() {
            return Err((SchedulerError::QueueError(QueueError::Full), tcb));
        }

        // 6. Ensure the task is in the Ready state
        if tcb.state() != TaskState::Ready {
            return Err((SchedulerError::InvalidTaskState(tcb.state()), tcb));
        }

        // 7. Perform ready queue insertion FIRST
        // If push_back fails, the task table remains unmodified, the slot reservation
        // remains held, and the uncommitted TCB is returned to the caller.
        let id = tcb.id;
        if let Err(q_err) = self.ready_queue.push_back(id) {
            return Err((SchedulerError::QueueError(q_err), tcb));
        }

        // 8. Queue insertion succeeded; insert TCB into the task table
        self.tasks[slot] = Some(tcb);

        // 9. Clear the reservation bit ONLY after all fallible steps succeed
        self.reserved_slots[slot] = false;

        Ok(())
    }

    /// Returns an immutable reference to the TCB for the given TaskId.
    pub fn get_task(&self, id: TaskId) -> Option<&TaskControlBlock> {
        let slot = self.find_task_slot(id)?;
        self.tasks[slot].as_ref()
    }

    /// Returns a mutable reference to the TCB for the given TaskId.
    pub fn get_task_mut(&mut self, id: TaskId) -> Option<&mut TaskControlBlock> {
        let slot = self.find_task_slot(id)?;
        self.tasks[slot].as_mut()
    }

    /// Returns the total number of registered tasks in the scheduler table.
    pub fn task_count(&self) -> usize {
        self.tasks.iter().filter(|entry| entry.is_some()).count()
    }

    /// Inserts a TaskControlBlock into a specific unoccupied slot.
    ///
    /// Zero heap memory allocation occurs during insertion.
    pub fn insert_task_at_slot(&mut self, slot: usize, tcb: TaskControlBlock) -> Result<(), SchedulerError> {
        if slot >= MAX_TASKS {
            return Err(SchedulerError::InvalidSlot(slot));
        }
        if self.tasks[slot].is_some() {
            return Err(SchedulerError::SlotOccupied(slot));
        }
        if self.find_task_slot(tcb.id).is_some() {
            return Err(SchedulerError::DuplicateTaskId(tcb.id));
        }
        self.tasks[slot] = Some(tcb);
        Ok(())
    }

    /// Removes a task from the table and returns ownership of its TaskControlBlock.
    ///
    /// Prevents the Idle Task and the currently executing task from being removed.
    /// Does NOT invoke heap deallocation while holding the scheduler lock.
    pub fn remove_task(&mut self, id: TaskId) -> Result<TaskControlBlock, SchedulerError> {
        if self.is_idle_task(id) {
            return Err(SchedulerError::CannotTerminateIdleTask);
        }
        if Some(id) == self.current_task_id {
            return Err(SchedulerError::CannotRemoveCurrentTask);
        }
        let slot = self.find_task_slot(id).ok_or(SchedulerError::TaskNotFound(id))?;
        Ok(self.tasks[slot].take().expect("Slot must be occupied"))
    }

    /// Transitions a task to Ready and enqueues it into the ready queue.
    ///
    /// Enforces the invariant that a task appears at most once in the ready queue.
    pub fn enqueue_ready(&mut self, id: TaskId) -> Result<(), SchedulerError> {
        let tcb = self.get_task_mut(id).ok_or(SchedulerError::TaskNotFound(id))?;
        if tcb.state() != TaskState::Ready {
            tcb.transition_to(TaskState::Ready)?;
        }
        self.ready_queue.push_back(id)?;
        Ok(())
    }

    /// Dequeues the next runnable task from the ready queue.
    pub fn dequeue_ready(&mut self) -> Result<TaskId, SchedulerError> {
        self.ready_queue.pop_front().map_err(SchedulerError::from)
    }

    /// Peeks at the next runnable task in the ready queue without removing it.
    pub fn peek_ready(&self) -> Result<TaskId, SchedulerError> {
        self.ready_queue.peek_front().map_err(SchedulerError::from)
    }

    /// Returns `true` if there are no runnable tasks in the ready queue.
    #[inline(always)]
    pub fn is_ready_empty(&self) -> bool {
        self.ready_queue.is_empty()
    }

    /// Returns the number of runnable tasks waiting in the ready queue.
    #[inline(always)]
    pub fn ready_count(&self) -> usize {
        self.ready_queue.len()
    }
}

/// Global kernel scheduler protected by an interrupt-safe Spinlock.
pub static SCHEDULER: Spinlock<Scheduler> = Spinlock::new(Scheduler::new());

/// Synthesizes the initial x86_64 stack frame for dynamic tasks.
///
/// Adheres strictly to the approved Stage 2 architecture and System V AMD64 ABI:
///
/// Frame Geometry (grows downwards from 16-byte aligned stack_top):
/// - `[stack_top - 0x08]`: 0 (dummy return address to terminate stack traces)
/// - `[stack_top - 0x10]`: entry_fn function pointer (popped into RIP by ret)
/// - `[stack_top - 0x18]`: INITIAL_RFLAGS_SANITIZED (0x0202: IF=1, Reserved Bit 1=1)
/// - `[stack_top - 0x20]`: r15 = 0
/// - `[stack_top - 0x28]`: r14 = 0
/// - `[stack_top - 0x30]`: r13 = 0
/// - `[stack_top - 0x38]`: r12 = 0
/// - `[stack_top - 0x40]`: rbx = 0
/// - `[stack_top - 0x48]`: rbp = 0
///
/// Total frame size: 72 bytes (9 quadwords).
/// Returns `initial_rsp = stack_top_aligned - 72`.
pub fn create_initial_task_frame(stack: &mut TaskStack, entry_fn: extern "C" fn()) -> usize {
    let stack_bottom = stack.bottom();
    let stack_top = stack_bottom.checked_add(TASK_STACK_SIZE).expect("Stack boundary calculation overflow");
    let stack_top_aligned = stack_top & !0xF;

    let frame_size = 9usize.checked_mul(core::mem::size_of::<usize>()).expect("Frame size overflow");
    let initial_rsp = stack_top_aligned.checked_sub(frame_size).expect("Stack frame underflow");

    assert!(initial_rsp >= stack_bottom, "Synthesized RSP below stack bottom");
    assert_eq!(initial_rsp % 8, 0, "Initial RSP must be 8-byte word aligned");

    unsafe {
        let stack_ptr = stack_top_aligned as *mut usize;
        // [stack_top - 0x08]: Dummy return address (offset -1)
        stack_ptr.offset(-1).write(0);
        // [stack_top - 0x10]: Function entry address for ret (offset -2)
        stack_ptr.offset(-2).write(entry_fn as usize);
        // [stack_top - 0x18]: Sanitized RFLAGS: IF=1, Reserved Bit 1=1 (offset -3)
        stack_ptr.offset(-3).write(INITIAL_RFLAGS_SANITIZED);
        // [stack_top - 0x20] .. [stack_top - 0x48]: Callee-saved registers (offsets -4 .. -9)
        stack_ptr.offset(-4).write(0); // r15
        stack_ptr.offset(-5).write(0); // r14
        stack_ptr.offset(-6).write(0); // r13
        stack_ptr.offset(-7).write(0); // r12
        stack_ptr.offset(-8).write(0); // rbx
        stack_ptr.offset(-9).write(0); // rbp
    }

    initial_rsp
}

/// Dynamically spawns a new task and registers it with the kernel scheduler.
///
/// # Strict Safety Invariants:
/// 1. Never allocates or deallocates heap memory while holding `SCHEDULER.lock()`.
/// 2. Uses a two-phase reservation protocol to prevent slot contention.
/// 3. In the event of heap allocation failure, cleanly releases the reserved slot.
/// 4. Constructs the initial stack frame according to the approved Stage 2 architecture.
/// 5. Enqueues the task's TaskId exactly once in the ready queue.
pub fn spawn(name: &'static str, entry_fn: extern "C" fn()) -> Result<TaskId, SpawnError> {
    // Phase 1: Under SCHEDULER lock — reserve a free slot and allocate monotonic TaskId
    let (slot, task_id) = {
        let mut sched = SCHEDULER.lock();
        sched.reserve_slot()?
    }; // Lock dropped!

    // Phase 2: Outside SCHEDULER lock — allocate 16 KiB heap stack
    let mut stack = match TaskStack::new_dynamic() {
        Some(s) => s,
        None => {
            // Roll back the slot reservation upon OOM
            let mut sched = SCHEDULER.lock();
            sched.release_reservation(slot);
            return Err(SpawnError::OutOfMemory);
        }
    };

    // Phase 3: Outside SCHEDULER lock — synthesize stack frame and construct TCB
    let initial_rsp = create_initial_task_frame(&mut *stack, entry_fn);
    let tcb = TaskControlBlock::new_dynamic(task_id, name, entry_fn, stack, initial_rsp);

    // Phase 4: Under SCHEDULER lock — commit task into table and enqueue into ready queue
    let commit_result = {
        let mut sched = SCHEDULER.lock();
        match sched.commit_spawned_task(slot, tcb) {
            Ok(()) => Ok(()),
            Err((err, uncommitted_tcb)) => {
                // Ensure the caller (spawn) properly releases the reservation if commit fails
                sched.release_reservation(slot);
                Err((err, uncommitted_tcb))
            }
        }
    }; // Lock dropped!

    // Handle commit outcome outside the scheduler lock
    match commit_result {
        Ok(()) => Ok(task_id),
        Err((err, uncommitted_tcb)) => {
            // Drop uncommitted_tcb strictly outside the scheduler lock to guarantee
            // that heap stack memory deallocation never occurs while holding SCHEDULER.lock()
            drop(uncommitted_tcb);
            Err(SpawnError::Scheduler(err))
        }
    }
}

/// Cooperatively yields the CPU from the currently running task to the next Ready task.
///
/// # Strict Safety Invariants:
/// 1. Pre-CLI flag capture: Captures the task's true runtime IF bit BEFORE acquiring
///    the scheduler lock (which disables interrupts).
/// 2. Atomically validates and transitions the outgoing task (Running -> Ready) and
///    incoming task (Ready -> Running) under `SCHEDULER.lock()`.
/// 3. Updates `current_task_id` consistently under `SCHEDULER.lock()`.
/// 4. Releases `SCHEDULER.lock()` BEFORE executing `switch_context` to prevent deadlocks.
/// 5. Restores the incoming task's sanitized RFLAGS via `popfq` inside assembly.
pub fn yield_now() {
    // 1. Pre-CLI Flag Capture: Read RFLAGS before taking the scheduler spinlock (which disables interrupts)
    let rflags = x86_64::registers::rflags::read_raw() as usize;
    let sanitized_rflags = (rflags & RFLAGS_IF_MASK) | RFLAGS_RESERVED;

    // 2. Select next eligible Ready task under SCHEDULER.lock()
    let switch_args: Option<(*mut usize, usize)> = {
        let mut sched = SCHEDULER.lock();

        // If no task is registered as running, adopt the calling execution context as TaskId(0) (Main)
        if sched.current_task_id.is_none() {
            let main_id = TaskId(0);
            let slot = match sched.find_free_slot() {
                Some(s) => s,
                None => return,
            };
            let main_tcb = TaskControlBlock::new_main(main_id, "Main", 0);
            sched.tasks[slot] = Some(main_tcb);
            sched.current_task_id = Some(main_id);
        }

        let current_id = sched.current_task_id.expect("Current task ID must be set");

        // Scan ready queue for the next valid, Ready task (skipping current_id and non-Ready tasks)
        let mut next_task_info = None;
        while let Ok(candidate_id) = sched.ready_queue.pop_front() {
            if candidate_id == current_id {
                // Already executing on CPU
                continue;
            }
            if let Some(slot) = sched.find_task_slot(candidate_id) {
                if let Some(ref tcb) = sched.tasks[slot] {
                    if tcb.state() == TaskState::Ready {
                        next_task_info = Some((candidate_id, slot));
                        break;
                    }
                }
            }
        }

        let (next_id, next_slot) = match next_task_info {
            Some(info) => info,
            None => {
                // No eligible Ready task found; current task retains CPU
                return;
            }
        };

        // Find current task slot
        let current_slot = match sched.find_task_slot(current_id) {
            Some(s) => s,
            None => return,
        };

        // Transition incoming task to Running and record stats
        {
            let next_tcb = sched.tasks[next_slot].as_mut().expect("Next TCB must exist");
            assert_eq!(
                next_tcb.state(),
                TaskState::Ready,
                "Candidate task must be in Ready state"
            );
            next_tcb.transition_to(TaskState::Running).expect("Ready -> Running transition failed");
            next_tcb.record_context_switch(crate::timer::current_ticks());
        }

        // Transition outgoing task to Ready and re-enqueue it
        {
            let current_tcb = sched.tasks[current_slot].as_mut().expect("Current TCB must exist");
            if current_tcb.state() == TaskState::Running {
                current_tcb.transition_to(TaskState::Ready).expect("Running -> Ready transition failed");
            }
            sched.ready_queue.push_back(current_id).expect("Failed to re-enqueue current task");
        }

        // Update current running task ID
        sched.set_current_task_id(Some(next_id));

        // Obtain raw pointers for stack switch
        let current_tcb = sched.tasks[current_slot].as_mut().unwrap();
        let old_rsp_ptr = &raw mut current_tcb.saved_rsp;
        let next_tcb = sched.tasks[next_slot].as_ref().unwrap();
        let new_rsp = next_tcb.saved_rsp;

        Some((old_rsp_ptr, new_rsp))
    }; // SCHEDULER lock is DROPPED here!

    // 3. Perform genuine CPU context switch strictly outside SCHEDULER lock
    if let Some((old_rsp_ptr, new_rsp)) = switch_args {
        unsafe {
            switch_context(old_rsp_ptr, new_rsp, sanitized_rflags);
        }
    }
}

/// Comprehensive verification test for Phase 2B Scheduler & ReadyQueue structures.
pub fn test_scheduler_structure() {
    log_info!("Starting Milestone 6 Stage 2B (Scheduler & ReadyQueue) test suite...");

    // 1. ReadyQueue empty state verification
    let mut q = ReadyQueue::new();
    assert!(q.is_empty(), "Queue must be empty upon initialization");
    assert_eq!(q.len(), 0);
    assert_eq!(q.pop_front(), Err(QueueError::Empty));
    assert_eq!(q.peek_front(), Err(QueueError::Empty));
    log_info!("  [Test 1] ReadyQueue empty state verified.");

    // 2. FIFO Enqueue & Dequeue
    assert_eq!(q.push_back(TaskId(1)), Ok(()));
    assert_eq!(q.push_back(TaskId(2)), Ok(()));
    assert_eq!(q.push_back(TaskId(3)), Ok(()));
    assert_eq!(q.len(), 3);
    assert_eq!(q.peek_front(), Ok(TaskId(1)));
    assert_eq!(q.pop_front(), Ok(TaskId(1)));
    assert_eq!(q.pop_front(), Ok(TaskId(2)));
    assert_eq!(q.pop_front(), Ok(TaskId(3)));
    assert!(q.is_empty());
    log_info!("  [Test 2] FIFO ordering and peek behavior verified.");

    // 3. Duplicate TaskId rejection
    assert_eq!(q.push_back(TaskId(42)), Ok(()));
    assert_eq!(q.push_back(TaskId(42)), Err(QueueError::DuplicateEntry(TaskId(42))));
    assert_eq!(q.len(), 1);
    assert_eq!(q.pop_front(), Ok(TaskId(42)));
    log_info!("  [Test 3] Duplicate TaskId rejection verified.");

    // 4. Queue capacity and full condition
    for i in 1..=MAX_TASKS {
        assert_eq!(q.push_back(TaskId(i)), Ok(()));
    }
    assert!(q.is_full());
    assert_eq!(q.len(), MAX_TASKS);
    assert_eq!(q.push_back(TaskId(999)), Err(QueueError::Full));
    for i in 1..=MAX_TASKS {
        assert_eq!(q.pop_front(), Ok(TaskId(i)));
    }
    assert!(q.is_empty());
    log_info!("  [Test 4] Full queue boundary and drain behavior verified.");

    // 5. Validated Lifecycle State Transitions
    let mut dummy = TaskControlBlock::new_main(TaskId(100), "Dummy", 0);
    assert_eq!(dummy.state(), TaskState::Running);

    // Running -> Ready (yield)
    assert_eq!(dummy.transition_to(TaskState::Ready), Ok(()));
    assert_eq!(dummy.state(), TaskState::Ready);

    // Ready -> Blocked is INVALID (must run before blocking)
    assert!(matches!(
        dummy.transition_to(TaskState::Blocked),
        Err(StateTransitionError::InvalidTransition { .. })
    ));

    // Ready -> Running (schedule)
    assert_eq!(dummy.transition_to(TaskState::Running), Ok(()));

    // Running -> Blocked (I/O wait)
    assert_eq!(dummy.transition_to(TaskState::Blocked), Ok(()));

    // Blocked -> Running is INVALID (must become Ready first)
    assert!(matches!(
        dummy.transition_to(TaskState::Running),
        Err(StateTransitionError::InvalidTransition { .. })
    ));

    // Blocked -> Ready (wake)
    assert_eq!(dummy.transition_to(TaskState::Ready), Ok(()));

    // Ready -> Running -> Terminated
    assert_eq!(dummy.transition_to(TaskState::Running), Ok(()));
    assert_eq!(dummy.transition_to(TaskState::Terminated), Ok(()));
    assert_eq!(dummy.state(), TaskState::Terminated);

    // Terminated CANNOT transition to any state!
    assert_eq!(
        dummy.transition_to(TaskState::Ready),
        Err(StateTransitionError::AlreadyTerminated)
    );
    assert_eq!(
        dummy.transition_to(TaskState::Running),
        Err(StateTransitionError::AlreadyTerminated)
    );
    assert_eq!(
        dummy.transition_to(TaskState::Blocked),
        Err(StateTransitionError::AlreadyTerminated)
    );

    // Redundant transition rejection
    let mut dummy2 = TaskControlBlock::new_main(TaskId(101), "Dummy2", 0);
    assert_eq!(
        dummy2.transition_to(TaskState::Running),
        Err(StateTransitionError::RedundantTransition)
    );
    log_info!("  [Test 5] Strict lifecycle state transition rules verified.");

    // 6. Scheduler Monotonic ID & Table Operations Under Spinlock
    {
        let mut sched = SCHEDULER.lock();
        let id1 = sched.allocate_task_id();
        let id2 = sched.allocate_task_id();
        let id3 = sched.allocate_task_id();
        assert!(id1 < id2 && id2 < id3, "Task IDs must be strictly monotonic");

        let slot = sched.find_free_slot().expect("Must have free slots");
        let tcb = TaskControlBlock::new_main(id1, "TestTask", 0);
        assert_eq!(sched.insert_task_at_slot(slot, tcb), Ok(()));
        assert_eq!(sched.task_count(), 1);

        // Idle task reservation
        assert_eq!(sched.set_idle_task(id1), Ok(()));
        assert!(sched.is_idle_task(id1));

        // Idle task cannot be removed or terminated
        assert_eq!(sched.remove_task(id1).err(), Some(SchedulerError::CannotTerminateIdleTask));

        // Unregister idle status and remove for test cleanup
        sched.idle_task_id = None;
        let removed = sched.remove_task(id1).expect("Must remove test task");
        assert_eq!(removed.id, id1);
        assert_eq!(sched.task_count(), 0);
    }
    log_info!("  [Test 6] Scheduler table, monotonic IDs, and idle task reservation verified.");

    log_info!("Milestone 6 Stage 2B test suite PASSED.");
}

/// Dummy task entry function used for spawn verification.
extern "C" fn dummy_test_task_entry() {
    loop {
        core::hint::spin_loop();
    }
}

/// Verification test suite for Milestone 6 Stage 2C (Dynamic Task Creation).
pub fn test_dynamic_task_creation() {
    log_info!("Starting Milestone 6 Stage 2C (Dynamic Task Creation) test suite...");

    // 1. Single dynamic task spawn
    let id_alpha = spawn("TaskAlpha", dummy_test_task_entry).expect("Failed to spawn TaskAlpha");
    assert!(id_alpha.as_usize() > 0, "TaskId must be positive");
    log_info!("  [Test 1] TaskAlpha spawned successfully with {}", id_alpha);

    // 2. Validate TCB in scheduler table
    {
        let sched = SCHEDULER.lock();
        let tcb = sched.get_task(id_alpha).expect("TaskAlpha must exist in scheduler");
        assert_eq!(tcb.name, "TaskAlpha");
        assert_eq!(tcb.state(), TaskState::Ready);
        assert!(tcb.stack.is_some(), "Dynamic task must own a heap stack");
        assert!(tcb.is_stack_within_bounds(tcb.saved_rsp), "Saved RSP must be within bounds");
        assert_eq!(tcb.switch_count, 0);
        assert_eq!(tcb.total_ticks, 0);
        assert!(sched.ready_queue.contains(id_alpha), "TaskAlpha must be in ready queue");
    }
    log_info!("  [Test 2] TaskAlpha TCB state, stack boundaries, and queue presence verified.");

    // 3. Multiple tasks spawn and monotonic IDs
    let id_beta = spawn("TaskBeta", dummy_test_task_entry).expect("Failed to spawn TaskBeta");
    let id_gamma = spawn("TaskGamma", dummy_test_task_entry).expect("Failed to spawn TaskGamma");
    assert!(id_alpha < id_beta && id_beta < id_gamma, "Spawned TaskIds must be strictly monotonic");
    log_info!("  [Test 3] Multiple tasks (Beta={}, Gamma={}) spawned with monotonic IDs.", id_beta, id_gamma);

    // 4. ReadyQueue ordering verification
    {
        let sched = SCHEDULER.lock();
        assert_eq!(sched.ready_count(), 3, "Ready queue must contain exactly 3 tasks");
        assert_eq!(sched.peek_ready(), Ok(id_alpha), "TaskAlpha must be at the front of ready queue");
    }
    log_info!("  [Test 4] ReadyQueue count and FIFO ordering verified.");

    // 5. Verification of Phase 2B required corrections:
    // a. Duplicate TaskId insertion rejection
    {
        let mut sched = SCHEDULER.lock();
        let slot = sched.find_free_slot().expect("Must have free slot");
        let duplicate_tcb = TaskControlBlock::new_main(id_alpha, "DupAlpha", 0);
        assert_eq!(
            sched.insert_task_at_slot(slot, duplicate_tcb),
            Err(SchedulerError::DuplicateTaskId(id_alpha)),
            "Duplicate TaskId insertion must be rejected"
        );
    }
    log_info!("  [Test 5a] Duplicate TaskId insertion rejection verified.");

    // b. Idle task replacement rejection
    {
        let mut sched = SCHEDULER.lock();
        assert_eq!(sched.set_idle_task(id_alpha), Ok(()));
        assert_eq!(
            sched.set_idle_task(id_beta),
            Err(SchedulerError::IdleTaskAlreadyRegistered),
            "Second idle task registration must be rejected"
        );
        sched.idle_task_id = None; // Reset idle task for test cleanup
    }
    log_info!("  [Test 5b] Idle task replacement rejection verified.");

    // c. Current running task removal protection
    {
        let mut sched = SCHEDULER.lock();
        sched.set_current_task_id(Some(id_alpha));
        assert_eq!(
            sched.remove_task(id_alpha).err(),
            Some(SchedulerError::CannotRemoveCurrentTask),
            "Cannot remove the currently executing task"
        );
        sched.set_current_task_id(None); // Reset current task
    }
    log_info!("  [Test 5c] Current task removal protection verified.");

    // d. Private state and getter verification
    {
        let sched = SCHEDULER.lock();
        let tcb = sched.get_task(id_alpha).unwrap();
        assert_eq!(tcb.state(), TaskState::Ready);
    }
    log_info!("  [Test 5d] Read-only state() getter verified.");

    // 6. Heap Allocation Failure & Rollback Test
    // Spawns tasks until heap is saturated and verifies OutOfMemory is handled cleanly
    let mut oom_spawned = Vec::new();
    let mut oom_detected = false;
    for _ in 0..10 {
        match spawn("OomTask", dummy_test_task_entry) {
            Ok(id) => oom_spawned.push(id),
            Err(SpawnError::OutOfMemory) => {
                oom_detected = true;
                break;
            }
            Err(e) => panic!("Unexpected spawn error: {:?}", e),
        }
    }
    assert!(oom_detected, "Must detect OutOfMemory on small 100 KiB heap");
    // Verify that after OOM, scheduler state is not corrupted and reserved slot was released
    {
        let sched = SCHEDULER.lock();
        // None of the free slots should remain stuck in reserved_slots
        for slot in 0..MAX_TASKS {
            if sched.tasks[slot].is_none() {
                assert!(!sched.reserved_slots[slot], "OOM must not leave slots in reserved state");
            }
        }
    }
    // Clean up OOM probe tasks
    {
        let mut sched = SCHEDULER.lock();
        for id in oom_spawned {
            let _ = sched.remove_task(id);
        }
    }
    log_info!("  [Test 6] Heap allocation failure (OutOfMemory) and reservation rollback verified.");

    // 7. Task Table Saturation Limit Test (TaskTableFull)
    // Fills remaining slots with static/main TCBs (zero heap memory) to test table boundary
    let mut filler_ids = Vec::new();
    {
        let mut sched = SCHEDULER.lock();
        let free_count = MAX_TASKS - sched.task_count();
        for _ in 0..free_count {
            let id = sched.allocate_task_id();
            let slot = sched.find_free_slot().expect("Must have free slot");
            let tcb = TaskControlBlock::new_main(id, "TableFiller", 0);
            sched.insert_task_at_slot(slot, tcb).expect("Filler insertion failed");
            filler_ids.push(id);
        }
        assert_eq!(sched.task_count(), MAX_TASKS, "Task table must be completely full");
    }

    // Now table is full (32 tasks): spawn must fail with TaskTableFull
    assert_eq!(
        spawn("OverflowTask", dummy_test_task_entry),
        Err(SpawnError::TaskTableFull),
        "Spawn must return TaskTableFull when table is saturated"
    );
    log_info!("  [Test 7] Task table saturation limit (TaskTableFull) verified.");

    // 8. Final Cleanup of all test tasks
    {
        let mut sched = SCHEDULER.lock();
        sched.ready_queue.clear();
        let _ = sched.remove_task(id_alpha);
        let _ = sched.remove_task(id_beta);
        let _ = sched.remove_task(id_gamma);
        for id in filler_ids {
            let _ = sched.remove_task(id);
        }
        assert_eq!(sched.task_count(), 0, "All test tasks must be cleanly removed");
        assert!(sched.is_ready_empty(), "Ready queue must be empty after cleanup");
    }
    log_info!("  [Test 8] Test tasks cleaned up; scheduler state restored.");

    // 9. Atomic Commit & Failure Recovery Tests (Stage 2C Corrections)
    // 9a. Successful task commit inserts exactly one task-table entry and one ready-queue entry
    {
        let mut sched = SCHEDULER.lock();
        let initial_task_count = sched.task_count();
        let initial_ready_count = sched.ready_count();

        let slot = sched.find_free_slot().expect("Must have free slot");
        sched.reserved_slots[slot] = true;
        let test_id = TaskId(9001);

        let mut stack = TaskStack::new_dynamic().expect("Must allocate stack");
        let initial_rsp = create_initial_task_frame(&mut *stack, dummy_test_task_entry);
        let tcb = TaskControlBlock::new_dynamic(test_id, "AtomicCommitSuccess", dummy_test_task_entry, stack, initial_rsp);

        let commit_res = sched.commit_spawned_task(slot, tcb);
        assert!(commit_res.is_ok(), "Commit must succeed when valid");

        // Invariants on success:
        // 1. Exactly one task-table entry added
        assert_eq!(sched.task_count(), initial_task_count + 1);
        assert!(sched.tasks[slot].is_some());
        assert_eq!(sched.tasks[slot].as_ref().unwrap().id, test_id);
        // 2. Exactly one ready-queue entry added
        assert_eq!(sched.ready_count(), initial_ready_count + 1);
        assert!(sched.ready_queue.contains(test_id));
        // 3. Reservation bit cleared
        assert!(!sched.reserved_slots[slot]);

        // Cleanup test task
        let removed = sched.remove_task(test_id).expect("Must remove test task");
        let dequeued = sched.dequeue_ready().expect("Must dequeue test task");
        assert_eq!(dequeued, test_id);
        assert_eq!(sched.task_count(), initial_task_count);
        assert_eq!(sched.ready_count(), initial_ready_count);

        drop(sched);
        drop(removed);
    }
    log_info!("  [Test 9a] Successful commit inserts exactly one table entry and one queue entry.");

    // 9b. Queue-full failure leaves task table unchanged and slot reserved
    {
        let mut sched = SCHEDULER.lock();

        // Fill the ready queue to MAX_TASKS (32 items)
        let mut filler_count = 0;
        while !sched.ready_queue.is_full() {
            let dummy_id = TaskId(8000 + sched.ready_queue.len());
            sched.ready_queue.push_back(dummy_id).expect("Queue push must succeed");
            filler_count += 1;
        }
        assert!(sched.ready_queue.is_full(), "Ready queue must be full");

        // Reserve a slot for a test task
        let test_slot = sched.find_free_slot().expect("Must have free slot");
        sched.reserved_slots[test_slot] = true;
        let test_id = TaskId(9999);

        // Create a dynamic TCB
        let mut stack = TaskStack::new_dynamic().expect("Must allocate stack");
        let initial_rsp = create_initial_task_frame(&mut *stack, dummy_test_task_entry);
        let tcb = TaskControlBlock::new_dynamic(test_id, "QueueFullTest", dummy_test_task_entry, stack, initial_rsp);

        // Attempt commit: must fail with QueueError(QueueError::Full)
        let commit_res = sched.commit_spawned_task(test_slot, tcb);
        assert!(commit_res.is_err(), "Commit must fail when ready queue is full");

        let (err, uncommitted_tcb) = commit_res.unwrap_err();
        assert_eq!(err, SchedulerError::QueueError(QueueError::Full));

        // Invariants:
        // 1. Task table slot must remain empty (unmodified)
        assert!(sched.tasks[test_slot].is_none(), "Task table slot must be unmodified on failure");
        // 2. Slot reservation must remain held (not cleared prematurely)
        assert!(sched.reserved_slots[test_slot], "Slot reservation must remain held on failure");
        // 3. Task must not be present in ready queue
        assert!(!sched.ready_queue.contains(test_id), "Task must not be present in ready queue");

        // Release reservation cleanly
        sched.release_reservation(test_slot);
        assert!(!sched.reserved_slots[test_slot], "Slot reservation must be cleanly releasable");

        // Drain the dummy tasks from ready queue
        for _ in 0..filler_count {
            let _ = sched.ready_queue.pop_front();
        }

        drop(sched); // Drop lock before dropping uncommitted_tcb!
        drop(uncommitted_tcb); // Stack deallocated outside lock!
    }
    log_info!("  [Test 9b] Queue-full failure leaves task table unchanged and slot reserved.");

    // 9c. Failed commit leaves slot reservation cleanly releasable
    {
        let mut sched = SCHEDULER.lock();
        let slot = sched.find_free_slot().expect("Must have free slot");
        sched.reserved_slots[slot] = true;
        let test_id = TaskId(9998);

        let mut stack = TaskStack::new_dynamic().expect("Must allocate stack");
        let initial_rsp = create_initial_task_frame(&mut *stack, dummy_test_task_entry);
        let tcb = TaskControlBlock::new_dynamic(test_id, "ReleasableTest", dummy_test_task_entry, stack, initial_rsp);

        // Pre-insert duplicate ID into table to force DuplicateTaskId failure
        let dup_slot = sched.find_free_slot().expect("Must have another slot");
        let dup_tcb = TaskControlBlock::new_main(test_id, "ExistingTask", 0);
        sched.tasks[dup_slot] = Some(dup_tcb);

        let commit_res = sched.commit_spawned_task(slot, tcb);
        assert!(commit_res.is_err());
        let (err, uncommitted_tcb) = commit_res.unwrap_err();
        assert_eq!(err, SchedulerError::DuplicateTaskId(test_id));

        // Reservation still held
        assert!(sched.reserved_slots[slot]);
        assert!(sched.tasks[slot].is_none());

        // Release reservation
        sched.release_reservation(slot);
        assert!(!sched.reserved_slots[slot]);

        // Cleanup duplicate dummy
        sched.tasks[dup_slot] = None;

        drop(sched);
        drop(uncommitted_tcb);
    }
    log_info!("  [Test 9c] Failed commit leaves slot reservation cleanly releasable.");

    // 9d. Task is not inserted if it is not in the Ready state
    {
        let mut sched = SCHEDULER.lock();
        let slot = sched.find_free_slot().expect("Must have free slot");
        sched.reserved_slots[slot] = true;
        let test_id = TaskId(9997);

        let mut stack = TaskStack::new_dynamic().expect("Must allocate stack");
        let initial_rsp = create_initial_task_frame(&mut *stack, dummy_test_task_entry);
        let mut tcb = TaskControlBlock::new_dynamic(test_id, "NonReadyTask", dummy_test_task_entry, stack, initial_rsp);

        // Transition task to Running state: state() is now Running, NOT Ready!
        tcb.transition_to(TaskState::Running).expect("Ready -> Running is valid transition");
        assert_eq!(tcb.state(), TaskState::Running);

        let commit_res = sched.commit_spawned_task(slot, tcb);
        assert!(commit_res.is_err());
        let (err, uncommitted_tcb) = commit_res.unwrap_err();
        assert_eq!(err, SchedulerError::InvalidTaskState(TaskState::Running));

        // Verify task table is untouched
        assert!(sched.tasks[slot].is_none(), "Task must not be inserted into table if not Ready");
        assert!(!sched.ready_queue.contains(test_id), "Task must not be queued if not Ready");
        assert!(sched.reserved_slots[slot], "Slot reservation remains held");

        sched.release_reservation(slot);
        assert!(!sched.reserved_slots[slot]);

        drop(sched);
        drop(uncommitted_tcb);
    }
    log_info!("  [Test 9d] Task not inserted if not in Ready state (table & queue unmodified).");

    // 9e. Memory is reclaimed if commit fails (no leaks, stack deallocated outside lock)
    {
        let stats_baseline = crate::allocator::ALLOCATOR.stats();

        // 1. Under lock: reserve slot
        let (slot, test_id) = {
            let mut sched = SCHEDULER.lock();
            let s = sched.find_free_slot().expect("Must have free slot");
            sched.reserved_slots[s] = true;
            let id = sched.allocate_task_id();
            (s, id)
        };

        // 2. Outside lock: allocate heap stack and synthesize TCB
        let mut stack = TaskStack::new_dynamic().expect("Must allocate stack");
        let initial_rsp = create_initial_task_frame(&mut *stack, dummy_test_task_entry);
        let tcb = TaskControlBlock::new_dynamic(test_id, "ReclaimTest", dummy_test_task_entry, stack, initial_rsp);

        let stats_allocated = crate::allocator::ALLOCATOR.stats();
        assert!(stats_allocated.total_allocated_bytes > stats_baseline.total_allocated_bytes, "Memory must be allocated for stack");

        // 3. Fill ready queue to simulate another task filling the queue during Phase 2/3
        let mut filler_count = 0;
        {
            let mut sched = SCHEDULER.lock();
            while !sched.ready_queue.is_full() {
                let dummy_id = TaskId(7000 + sched.ready_queue.len());
                sched.ready_queue.push_back(dummy_id).expect("Queue push");
                filler_count += 1;
            }
        }

        // 4. Under lock: commit fails due to full ready queue
        let (err, uncommitted_tcb) = {
            let mut sched = SCHEDULER.lock();
            let res = sched.commit_spawned_task(slot, tcb);
            sched.release_reservation(slot);
            res.unwrap_err()
        }; // Lock dropped!

        assert_eq!(err, SchedulerError::QueueError(QueueError::Full));

        // 5. Outside lock: drop uncommitted_tcb (reclaiming heap stack)
        drop(uncommitted_tcb);

        // 6. Drain filler queue items
        {
            let mut sched = SCHEDULER.lock();
            for _ in 0..filler_count {
                let _ = sched.ready_queue.pop_front();
            }
        }

        let stats_after = crate::allocator::ALLOCATOR.stats();
        assert_eq!(
            stats_after.total_allocated_bytes,
            stats_baseline.total_allocated_bytes,
            "Heap memory must be completely reclaimed on commit failure!"
        );
    }
    log_info!("  [Test 9e] Memory reclamation on commit failure verified (zero leak).");

    // Final state verification
    {
        let sched = SCHEDULER.lock();
        assert_eq!(sched.task_count(), 0, "Task table must be empty after all tests");
        assert!(sched.is_ready_empty(), "Ready queue must be empty after all tests");
        for slot in 0..MAX_TASKS {
            assert!(!sched.reserved_slots[slot], "No slot should remain reserved");
        }
    }
    log_info!("  [Final] Scheduler state fully restored to baseline.");

    log_info!("Milestone 6 Stage 2C test suite PASSED.");
}

static mut TASK1_STACK: TaskStack = TaskStack([0; TASK_STACK_SIZE]);
static mut TASK2_STACK: TaskStack = TaskStack([0; TASK_STACK_SIZE]);

static mut MAIN_RSP: usize = 0;
static mut TASK1_RSP: usize = 0;
static mut TASK2_RSP: usize = 0;

static mut TASK1_FINAL: u64 = 0;
static mut TASK2_FINAL: u64 = 0;

/// Initializes a task stack to the exact layout expected by `switch_context`.
///
/// # Initial Stack Layout (System V AMD64 ABI):
///
/// Stack grows downwards from `stack_top` (16-byte aligned).
///
/// `[stack_top - 0x08]`: 0 (dummy return address to terminate stack traces)
/// `[stack_top - 0x10]`: `entry_fn` function pointer (popped into RIP by `ret`)
/// `[stack_top - 0x18]`: INITIAL_RFLAGS_SANITIZED (0x0202: IF=1, Reserved Bit 1=1)
/// `[stack_top - 0x20]`: r15 = 0
/// `[stack_top - 0x28]`: r14 = 0
/// `[stack_top - 0x30]`: r13 = 0
/// `[stack_top - 0x38]`: r12 = 0
/// `[stack_top - 0x40]`: rbx = 0
/// `[stack_top - 0x48]`: rbp = 0
///
/// Returns `initial_rsp = stack_top - 72`.
/// On entry to `entry_fn`, `RSP % 16 == 8`, identical to being called via `call`.
pub unsafe fn create_initial_stack(stack: *mut TaskStack, entry_fn: extern "C" fn()) -> usize {
    let stack_bottom = (*stack).0.as_mut_ptr() as usize;
    let stack_top = stack_bottom + TASK_STACK_SIZE;
    let stack_top_aligned = stack_top & !0xF;

    let stack_ptr = stack_top_aligned as *mut usize;

    // [stack_top - 0x08]: Dummy return address
    stack_ptr.offset(-1).write(0);
    // [stack_top - 0x10]: Function pointer for ret
    stack_ptr.offset(-2).write(entry_fn as usize);
    // [stack_top - 0x18]: Sanitized RFLAGS: IF=1, Reserved Bit 1=1
    stack_ptr.offset(-3).write(INITIAL_RFLAGS_SANITIZED);
    // [stack_top - 0x20] .. [stack_top - 0x48]: Callee-saved registers (R15..RBP)
    stack_ptr.offset(-4).write(0); // r15
    stack_ptr.offset(-5).write(0); // r14
    stack_ptr.offset(-6).write(0); // r13
    stack_ptr.offset(-7).write(0); // r12
    stack_ptr.offset(-8).write(0); // rbx
    stack_ptr.offset(-9).write(0); // rbp

    stack_top_aligned - 72
}

/// Entry point for Task 1 in the cooperative switching test.
extern "C" fn task1_entry() {
    let mut task1_counter: u64 = 100;

    for iter in 1..=3 {
        log_info!(
            "  [Task 1] Iteration {} | local counter = {} | yielding to Task 2...",
            iter,
            task1_counter
        );
        task1_counter += 10;

        // Yield to Task 2
        unsafe {
            switch_context(&raw mut TASK1_RSP, TASK2_RSP, INITIAL_RFLAGS_SANITIZED);
        }

        log_info!(
            "  [Task 1] Resumed! Local counter retained value = {}",
            task1_counter
        );
    }

    // Yield to Task 2 so it can complete its final iteration
    unsafe {
        switch_context(&raw mut TASK1_RSP, TASK2_RSP, INITIAL_RFLAGS_SANITIZED);
    }

    unsafe {
        core::ptr::write_volatile(&raw mut TASK1_FINAL, task1_counter);
        // All iterations complete; return control to Main Kernel context
        log_info!("  [Task 1] Completed all iterations! Yielding back to Main Kernel...");
        switch_context(&raw mut TASK1_RSP, MAIN_RSP, INITIAL_RFLAGS_SANITIZED);
    }

    loop {
        core::hint::spin_loop();
    }
}

/// Entry point for Task 2 in the cooperative switching test.
extern "C" fn task2_entry() {
    let mut task2_counter: u64 = 2000;

    for iter in 1..=3 {
        log_info!(
            "  [Task 2] Iteration {} | local counter = {} | yielding to Task 1...",
            iter,
            task2_counter
        );
        task2_counter += 50;

        // Yield to Task 1
        unsafe {
            switch_context(&raw mut TASK2_RSP, TASK1_RSP, INITIAL_RFLAGS_SANITIZED);
        }

        log_info!(
            "  [Task 2] Resumed! Local counter retained value = {}",
            task2_counter
        );
    }

    unsafe {
        core::ptr::write_volatile(&raw mut TASK2_FINAL, task2_counter);
        // All iterations complete; yield to Task 1 so it can finalize and return to Main
        log_info!("  [Task 2] Completed all iterations! Yielding to Task 1 for final return...");
        switch_context(&raw mut TASK2_RSP, TASK1_RSP, INITIAL_RFLAGS_SANITIZED);
    }

    loop {
        core::hint::spin_loop();
    }
}

/// Runs a verification test of genuine cooperative context switching.
///
/// Demonstrates:
/// 1. Main Kernel context yields to Task 1.
/// 2. Task 1 yields to Task 2 across 3 round-trip iterations.
/// 3. Both tasks maintain distinct local stack variables (`task1_counter`, `task2_counter`)
///    across yields without corruption.
/// 4. Task 1 and Task 2 complete and return cleanly to the Main Kernel context.
pub fn test_cooperative_context_switching() {
    log_info!("Initializing cooperative task switching test (Task 1 <-> Task 2)...");

    let final1: u64;
    let final2: u64;

    unsafe {
        TASK1_FINAL = 0;
        TASK2_FINAL = 0;
        TASK1_RSP = create_initial_stack(&raw mut TASK1_STACK, task1_entry);
        TASK2_RSP = create_initial_stack(&raw mut TASK2_STACK, task2_entry);

        // Switch from Main Kernel context to Task 1
        switch_context(&raw mut MAIN_RSP, TASK1_RSP, INITIAL_RFLAGS_SANITIZED);

        final1 = core::ptr::read_volatile(&raw const TASK1_FINAL);
        final2 = core::ptr::read_volatile(&raw const TASK2_FINAL);
        // Verification after both tasks finish and return to Main
        assert_eq!(
            final1, 130,
            "Task 1 local variable corrupted across yields"
        );
        assert_eq!(
            final2, 2150,
            "Task 2 local variable corrupted across yields"
        );
    }

    log_info!(
        "Cooperative context switching test PASSED (Task 1 final={}, Task 2 final={}).",
        final1,
        final2
    );
}

/// Global atomic counters recording demonstration task completions.
static DEMO_A_FINAL: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static DEMO_B_FINAL: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
static DEMO_C_FINAL: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Demonstration Task Alpha:
/// - Owns local stack variable `local_counter` initialized to 100.
/// - Increments by 10 across 3 iterations, calling `yield_now()` each time.
/// - Verifies state retention across yields.
extern "C" fn demo_task_alpha() {
    let mut local_counter: u64 = 100;

    for iter in 1..=3 {
        log_info!(
            "  [TaskAlpha] Iteration {}/3 | local_var = {} | calling yield_now()...",
            iter,
            local_counter
        );
        local_counter += 10;
        yield_now();
        log_info!(
            "  [TaskAlpha] Resumed! local_var = {} (state retained across yield)",
            local_counter
        );
    }

    DEMO_A_FINAL.store(local_counter, core::sync::atomic::Ordering::SeqCst);
    log_info!("  [TaskAlpha] Completed all iterations (final = {}). Yielding indefinitely...", local_counter);

    loop {
        yield_now();
    }
}

/// Demonstration Task Beta:
/// - Owns local stack variable `local_counter` initialized to 2000.
/// - Increments by 50 across 3 iterations, calling `yield_now()` each time.
/// - Verifies state retention across yields.
extern "C" fn demo_task_beta() {
    let mut local_counter: u64 = 2000;

    for iter in 1..=3 {
        log_info!(
            "  [TaskBeta] Iteration {}/3 | local_var = {} | calling yield_now()...",
            iter,
            local_counter
        );
        local_counter += 50;
        yield_now();
        log_info!(
            "  [TaskBeta] Resumed! local_var = {} (state retained across yield)",
            local_counter
        );
    }

    DEMO_B_FINAL.store(local_counter, core::sync::atomic::Ordering::SeqCst);
    log_info!("  [TaskBeta] Completed all iterations (final = {}). Yielding indefinitely...", local_counter);

    loop {
        yield_now();
    }
}

/// Demonstration Task Gamma:
/// - Owns local stack variable `local_counter` initialized to 30000.
/// - Increments by 100 across 3 iterations, calling `yield_now()` each time.
/// - Verifies state retention across yields.
extern "C" fn demo_task_gamma() {
    let mut local_counter: u64 = 30000;

    for iter in 1..=3 {
        log_info!(
            "  [TaskGamma] Iteration {}/3 | local_var = {} | calling yield_now()...",
            iter,
            local_counter
        );
        local_counter += 100;
        yield_now();
        log_info!(
            "  [TaskGamma] Resumed! local_var = {} (state retained across yield)",
            local_counter
        );
    }

    DEMO_C_FINAL.store(local_counter, core::sync::atomic::Ordering::SeqCst);
    log_info!("  [TaskGamma] Completed all iterations (final = {}). Yielding indefinitely...", local_counter);

    loop {
        yield_now();
    }
}

/// Verification test suite for Milestone 6 Stage 2D (Scheduler-Driven Context Switching).
pub fn test_scheduler_context_switching() {
    log_info!("Starting Milestone 6 Stage 2D (Scheduler-Driven Context Switching) test suite...");

    // 1. Spawn three independent tasks on dynamic heap stacks
    let id_alpha = spawn("TaskAlpha", demo_task_alpha).expect("Failed to spawn TaskAlpha");
    let id_beta = spawn("TaskBeta", demo_task_beta).expect("Failed to spawn TaskBeta");
    let id_gamma = spawn("TaskGamma", demo_task_gamma).expect("Failed to spawn TaskGamma");

    log_info!(
        "  [Spawn] Successfully spawned 3 tasks: Alpha={}, Beta={}, Gamma={}",
        id_alpha,
        id_beta,
        id_gamma
    );

    // 2. Execute 4 rounds of cooperative yields from the Main kernel context
    for round in 1..=4 {
        log_info!("  [Main] Round {}/4: Yielding CPU to ready tasks...", round);
        yield_now();
        log_info!("  [Main] Round {}/4: CPU returned to Main kernel context.", round);
    }

    // 3. Verify all 3 tasks reached their expected final local counter values
    let final_a = DEMO_A_FINAL.load(core::sync::atomic::Ordering::SeqCst);
    let final_b = DEMO_B_FINAL.load(core::sync::atomic::Ordering::SeqCst);
    let final_c = DEMO_C_FINAL.load(core::sync::atomic::Ordering::SeqCst);

    assert_eq!(final_a, 130, "TaskAlpha local stack counter mismatch");
    assert_eq!(final_b, 2150, "TaskBeta local stack counter mismatch");
    assert_eq!(final_c, 30300, "TaskGamma local stack counter mismatch");

    log_info!(
        "  [Verification] Final counter values verified: Alpha={}, Beta={}, Gamma={}",
        final_a,
        final_b,
        final_c
    );

    // 4. Verify TCB execution statistics
    {
        let sched = SCHEDULER.lock();
        let tcb_a = sched.get_task(id_alpha).expect("TaskAlpha must exist");
        let tcb_b = sched.get_task(id_beta).expect("TaskBeta must exist");
        let tcb_c = sched.get_task(id_gamma).expect("TaskGamma must exist");

        assert!(tcb_a.switch_count >= 3, "TaskAlpha switch_count must be >= 3");
        assert!(tcb_b.switch_count >= 3, "TaskBeta switch_count must be >= 3");
        assert!(tcb_c.switch_count >= 3, "TaskGamma switch_count must be >= 3");

        log_info!(
            "  [Stats] Context switch counts: Alpha={}, Beta={}, Gamma={}",
            tcb_a.switch_count,
            tcb_b.switch_count,
            tcb_c.switch_count
        );
    }

    // Clean up test tasks so they do not remain as zombie tasks constantly context switching
    {
        let mut sched = SCHEDULER.lock();
        sched.ready_queue.clear();
        let _ = sched.remove_task(id_alpha);
        let _ = sched.remove_task(id_beta);
        let _ = sched.remove_task(id_gamma);
    }

    log_info!("Milestone 6 Stage 2D test suite PASSED.");
}
