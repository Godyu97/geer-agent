use std::cell::RefCell;

thread_local! {
    static BUFFER: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

pub(crate) fn emit_diagnostic(message: impl AsRef<str>) {
    let message = message.as_ref();
    let buffered = BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        if let Some(lines) = buffer.as_mut() {
            lines.push(message.to_owned());
            true
        } else {
            false
        }
    });
    if !buffered {
        eprintln!("{message}");
    }
}

pub(crate) struct DiagnosticBuffer {
    active: bool,
}

impl DiagnosticBuffer {
    pub(crate) fn start() -> Self {
        BUFFER.with(|buffer| *buffer.borrow_mut() = Some(Vec::new()));
        Self { active: true }
    }

    #[cfg(any(feature = "gui", feature = "web"))]
    pub(crate) fn drain(&self) -> Vec<String> {
        BUFFER.with(|buffer| {
            buffer
                .borrow_mut()
                .as_mut()
                .map(std::mem::take)
                .unwrap_or_default()
        })
    }

    pub(crate) fn finish(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let lines = BUFFER.with(|buffer| buffer.borrow_mut().take().unwrap_or_default());
        for line in lines {
            eprintln!("{line}");
        }
    }
}

impl Drop for DiagnosticBuffer {
    fn drop(&mut self) {
        self.finish();
    }
}
