namespace rs terminal.streaming_interface
namespace py terminal.streaming_interface

struct OutputChunk {
  1: optional string chunk
}

enum OutputType {
  STDOUT = 0,
  STDERR = 1,
}

struct OutputEvent {
  1: optional OutputType type
  2: optional OutputChunk chunk
}

enum EventType {
  START = 0,
  FIN = 1,
  OUTPUT = 2,
}

struct TimingWithinRun {
  // Higher-resolution time relative to the start of the run.
  1: optional i64 milliseconds_since_start_of_run
}

struct RunId {
  1: optional string id
}

struct ExitStatus {
  1: optional i32 exit_code
}

struct SubprocessEvent {
  1: optional EventType type
  2: optional TimingWithinRun timing
  3: optional RunId run_id
  4: optional ExitStatus exit_status
}

// There is assumed to be some mapping from RunId -> ProcessExecutionRequest elsewhere!
struct ProcessExecutionRequest {
  1: optional list<string> argv
  2: optional map<string, string> env
  // The absolute time of when the run begins, in seconds.
  3: optional i64 unix_epoch_seconds
  4: optional string cwd
}

exception CreationError {
  1: optional string description
}

exception ExecutionError {
  1: optional string description
}

exception ProgramHasEnded {
  1: optional i32 exit_code
  2: optional TimingWithinRun when_it_was_completed
}

service TerminalWrapper {
  RunId beginExecution(1: ProcessExecutionRequest exe_req)
    throws (1: CreationError creation_error)

  SubprocessEvent getNextEvent()
    throws (1: ProgramHasEnded program_has_ended
            2: ExecutionError exe_error)
}
