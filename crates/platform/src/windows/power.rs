//! Processor frequency via `CallNtPowerInformation(ProcessorInformation)`.

use ::windows::Win32::Foundation::STATUS_BUFFER_TOO_SMALL;
use ::windows::Win32::System::Power::{
    CallNtPowerInformation, PROCESSOR_POWER_INFORMATION, ProcessorInformation,
};
use ::windows::Win32::System::Threading::{ALL_PROCESSOR_GROUPS, GetActiveProcessorCount};
use models::{AppError, ErrorCode};

/// Active logical processors across all processor groups (0 on failure).
pub fn logical_processor_count() -> usize {
    // SAFETY: no pointers involved.
    unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) as usize }
}

/// One entry per logical processor (all processor groups).
pub fn processor_power_info() -> Result<Vec<PROCESSOR_POWER_INFORMATION>, AppError> {
    let mut count = logical_processor_count().max(1);
    for _ in 0..3 {
        let mut buf = vec![PROCESSOR_POWER_INFORMATION::default(); count];
        let bytes = u32::try_from(std::mem::size_of_val(buf.as_slice())).unwrap_or(u32::MAX);
        // SAFETY: the output buffer is `bytes` long and correctly typed; no input buffer.
        let status = unsafe {
            CallNtPowerInformation(
                ProcessorInformation,
                None,
                0,
                Some(buf.as_mut_ptr().cast()),
                bytes,
            )
        };
        if status == STATUS_BUFFER_TOO_SMALL {
            count *= 2;
            continue;
        }
        if status.0 != 0 {
            return Err(AppError::new(
                ErrorCode::Platform,
                "CallNtPowerInformation(ProcessorInformation) failed",
            )
            .with_detail(format!("NTSTATUS 0x{:08X}", status.0)));
        }
        return Ok(buf);
    }
    Err(AppError::new(
        ErrorCode::Platform,
        "CallNtPowerInformation buffer too small after retries",
    ))
}
