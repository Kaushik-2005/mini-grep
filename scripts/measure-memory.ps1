param(
    [switch]$Large,
    [switch]$LongLine
)

$ErrorActionPreference = 'Stop'

$grepRoot = Split-Path -Parent $PSScriptRoot
$grepExecutable = Join-Path $grepRoot 'target\release\mini-grep.exe'
if ($Large -and $LongLine) {
    throw 'Use either -Large or -LongLine.'
}

$grepMultiplier = if ($Large) { 8 } else { 1 }
$grepInputName = if ($LongLine) {
    'profile-input-long-line.txt'
} elseif ($Large) {
    'profile-input-large.txt'
} else {
    'profile-input.txt'
}
$grepInput = Join-Path $grepRoot "target\profiling\$grepInputName"
$grepExpectedBytes = 17285120L * $grepMultiplier
$grepExpectedCount = if ($LongLine) {
    1L
} else {
    8192L * $grepMultiplier
}

if (-not (Test-Path -LiteralPath $grepExecutable -PathType Leaf)) {
    throw 'Build the release executable first.'
}

if (-not (Test-Path -LiteralPath $grepInput -PathType Leaf)) {
    throw 'The verified profiling input is missing.'
}

if ((Get-Item -LiteralPath $grepInput).Length -ne $grepExpectedBytes) {
    throw 'The profiling input has an unexpected size.'
}

if (-not ('MiniGrepMemoryNative' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class MiniGrepMemoryNative
{
    [StructLayout(LayoutKind.Sequential)]
    private struct MemoryCounters
    {
        public uint Size;
        public uint PageFaultCount;
        public UIntPtr PeakWorkingSetSize;
        public UIntPtr WorkingSetSize;
        public UIntPtr QuotaPeakPagedPoolUsage;
        public UIntPtr QuotaPagedPoolUsage;
        public UIntPtr QuotaPeakNonPagedPoolUsage;
        public UIntPtr QuotaNonPagedPoolUsage;
        public UIntPtr PagefileUsage;
        public UIntPtr PeakPagefileUsage;
    }

    [DllImport("psapi.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetProcessMemoryInfo(
        IntPtr process,
        ref MemoryCounters counters,
        uint size
    );

    public static ulong ReadPeakWorkingSet(IntPtr process)
    {
        var counters = new MemoryCounters();
        counters.Size = (uint)Marshal.SizeOf(typeof(MemoryCounters));

        if (!GetProcessMemoryInfo(
            process, ref counters, counters.Size))
        {
            throw new Win32Exception(Marshal.GetLastWin32Error());
        }

        ulong peak = counters.PeakWorkingSetSize.ToUInt64();

        if (peak == 0)
        {
            throw new InvalidOperationException(
                "Windows returned a zero peak; discard this measurement.");
        }

        return peak;
    }
}
'@
}

$grepResults = foreach ($grepRun in 1..5) {
    $grepStartInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $grepStartInfo.FileName = $grepExecutable
    $grepStartInfo.UseShellExecute = $false
    $grepStartInfo.RedirectStandardOutput = $true
    $grepStartInfo.RedirectStandardError = $true

    foreach ($grepArgument in @(
        '-F', '-c', '-H', '--color=never', '--text',
        '-j1', 'Rust', $grepInput
    )) {
        $grepStartInfo.ArgumentList.Add($grepArgument)
    }

    $grepProcess = [System.Diagnostics.Process]::new()
    $grepProcess.StartInfo = $grepStartInfo

    try {
        if (-not $grepProcess.Start()) {
            throw 'Search process could not start.'
        }

        # Retain the process handle until memory counters are read.
        $grepHandle = $grepProcess.Handle
        $grepOutputTask = $grepProcess.StandardOutput.ReadToEndAsync()
        $grepErrorTask = $grepProcess.StandardError.ReadToEndAsync()

        $grepProcess.WaitForExit()

        $grepPeak = [MiniGrepMemoryNative]::ReadPeakWorkingSet(
            $grepHandle
        )

        $grepOutput = $grepOutputTask.GetAwaiter().GetResult()
        $grepError = $grepErrorTask.GetAwaiter().GetResult()

        if ($grepProcess.ExitCode -ne 0) {
            throw "Search failed: $grepError"
        }

        if ($grepError.Length -ne 0) {
            throw "Unexpected diagnostics: $grepError"
        }

        if ($grepOutput.TrimEnd("`r", "`n") -ne "${grepInput}:$grepExpectedCount") {
            throw "Unexpected count output: $grepOutput"
        }

        [pscustomobject]@{
            Run = $grepRun
            PeakWorkingSetBytes = $grepPeak
            PeakWorkingSetMiB = [math]::Round($grepPeak / 1MB, 3)
        }
    } finally {
        $grepProcess.Dispose()
    }
}

$grepResults | Format-Table -AutoSize
