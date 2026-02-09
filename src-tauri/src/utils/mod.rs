// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Utility modules providing cross-cutting concerns like platform
// detection, archive extraction, and subprocess output parsing.

/// Platform detection, path resolution, and OS-specific utilities
pub mod platform;

/// Archive extraction utilities (ZIP, TAR.GZ, TAR.XZ)
pub mod archive;

/// Subprocess stdout/stderr parsing for GAMDL output
pub mod process;
