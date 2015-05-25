use call_frame::CallFrame;
use compiled_code::CompiledCode;
use heap::Heap;
use std::mem;

pub struct Thread<'l> {
    pub call_frame: CallFrame<'l>,
    pub young_heap: Heap<'l>,
    pub mature_heap: Heap<'l>
}

impl<'l> Thread<'l> {
    pub fn new(call_frame: CallFrame<'l>) -> Thread<'l> {
        Thread {
            call_frame: call_frame,
            young_heap: Heap::new(),
            mature_heap: Heap::new()
        }
    }

    pub fn add_call_frame_from_compiled_code(&mut self, code: &CompiledCode<'l>) {
        let mut frame = CallFrame::new(code.name, code.file, code.line);

        mem::swap(&mut self.call_frame, &mut frame);

        self.call_frame.set_parent(frame);
    }

    pub fn pop_call_frame(&mut self) {
        let parent = self.call_frame.parent.take().unwrap();

        // TODO: this might move the data from heap back to the stack?
        self.call_frame = *parent;
    }
}
