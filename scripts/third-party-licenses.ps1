# Writes THIRD_PARTY_LICENSES.md: every third-party crate compiled into the
# Windows ORNATR.exe (from Cargo.lock), with its license text, taken from
# the crate sources in the local Cargo registry. Run after dependencies change:
#   powershell -ExecutionPolicy Bypass -File scripts\third-party-licenses.ps1
# Needs one build (or `cargo fetch`) first, so the crate sources are present.
# Where a crate offers a choice that includes MIT, the MIT terms are used.
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
$m = cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc --locked | ConvertFrom-Json
$pk = @{}; foreach ($p in $m.packages) { $pk[$p.id] = $p }
$nodes = @{}; foreach ($n in $m.resolve.nodes) { $nodes[$n.id] = $n }

# crates reachable from the program through normal (non-dev, non-build) dependencies
$root = ($m.packages | Where-Object { $_.name -eq 'ornatr' }).id
$seen = @{}; $stack = New-Object System.Collections.Stack; $stack.Push($root)
while ($stack.Count) {
    $id = $stack.Pop(); if ($seen[$id]) { continue }; $seen[$id] = $true
    foreach ($d in $nodes[$id].deps) { if ($d.dep_kinds | Where-Object { $null -eq $_.kind }) { $stack.Push($d.pkg) } }
}
$crates = $seen.Keys | ForEach-Object { $pk[$_] } | Where-Object { $_.source } | Sort-Object name, version

$licenseName = '(?i)^(licen[cs]e|copying|notice|unlicense|copyright|ofl|ufl)'
function LicenseFiles($dir) {
    $files = @(Get-ChildItem -LiteralPath $dir -File | Where-Object { $_.Name -match $licenseName })
    # fonts bundled by epaint_default_fonts keep their licenses beside them
    if (Test-Path (Join-Path $dir 'fonts')) { $files += Get-ChildItem -LiteralPath (Join-Path $dir 'fonts') -File -Filter *.txt }
    $files | Sort-Object Name
}
function Read($f) { ((Get-Content -LiteralPath $f.FullName -Raw -Encoding UTF8) -replace "`r", '').Trim() }

$apache = $null
$entries = [ordered]@{}   # license text -> list of crates
$rows = @()
foreach ($c in $crates) {
    $dir = Split-Path $c.manifest_path
    $expr = if ($c.license) { $c.license } else { "see $($c.license_file)" }
    $files = @(LicenseFiles $dir)
    foreach ($f in $files) { if (-not $apache -and $f.Name -match '(?i)apache') { $apache = Read $f } }
    $choice = $null; $parts = @()
    $alternatives = $expr -notmatch '\bAND\b'
    if ($alternatives -and $expr -match '\bMIT\b') {
        $choice = 'MIT'
        $mit = $files | Where-Object { $_.Name -match '(?i)mit' } | Select-Object -First 1
        if (-not $mit) { $mit = $files | Where-Object { (Read $_) -match 'Permission is hereby granted' } | Select-Object -First 1 }
        if ($mit) { $parts += Read $mit }
        else { $parts += "MIT License`n`nCopyright (c) $(($c.authors -join ', '))`n`n(No license file is included in this crate's package; the standard MIT terms below apply.)`n`n@@MIT@@" }
    } elseif ($alternatives -and $expr -match 'Apache-2\.0') {
        $choice = 'Apache-2.0'
        $notices = @($files | Where-Object { $_.Name -match '(?i)^notice' })
        $parts += "Licensed under the Apache License, Version 2.0 (full text at the end of this file)."
        foreach ($n in $notices) { $parts += "$($n.Name):`n`n$(Read $n)" }
    } else {
        $choice = $expr
        foreach ($f in $files) { $parts += "$($f.Name):`n`n$(Read $f)" }
        if (-not $files) {
            if ($expr -eq 'BSL-1.0') { $parts += "Boost Software License - Version 1.0 (no license file is included in this crate's package; the standard text follows)`n`n@@BSL@@" }
            elseif ($expr -eq 'CC0-1.0') { $parts += "Dedicated to the public domain under CC0 1.0 Universal (https://creativecommons.org/publicdomain/zero/1.0/); no notice is required." }
            else { $parts += "License: $expr (no license file is included in this crate's package)." }
        }
    }
    $text = $parts -join "`n`n"
    if (-not $entries.Contains($text)) { $entries[$text] = @() }
    $entries[$text] += "$($c.name) $($c.version)"
    $rows += "| $($c.name) | $($c.version) | $expr | $choice |"
}

$mitText = @'
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
'@

$bslText = @'
Permission is hereby granted, free of charge, to any person or organization
obtaining a copy of the software and accompanying documentation covered by
this license (the "Software") to use, reproduce, display, distribute,
execute, and transmit the Software, and to prepare derivative works of the
Software, and to permit third-parties to whom the Software is furnished to
do so, all subject to the following:

The copyright notices in the Software and this entire statement, including
the above license grant, this restriction and the following disclaimer,
must be included in all copies of the Software, in whole or in part, and
all derivative works of the Software, unless such copies or derivative
works are solely in the form of machine-executable object code generated by
a source language processor.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT
SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE
FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
'@

$out = New-Object System.Text.StringBuilder
[void]$out.AppendLine('# Third-party licenses in ORNATR.exe')
[void]$out.AppendLine()
[void]$out.AppendLine("ORNATR.exe (Windows) is built from ORNATR's own source (GPL-3.0-only) together with the $($crates.Count) third-party Rust crates listed below, pinned in ``Cargo.lock``. Each keeps its own license. Where a crate offers a choice of licenses that includes MIT, ORNATR uses it under the MIT terms. Crates used only while building (build scripts and tests) are not part of the program and are not listed; procedural-macro crates are listed although only their output is compiled in.")
[void]$out.AppendLine()
[void]$out.AppendLine('Generated by `scripts/third-party-licenses.ps1` from the crate sources in the Cargo registry. Regenerate it whenever `Cargo.lock` changes. This is an inventory, not a legal certification.')
[void]$out.AppendLine()
[void]$out.AppendLine('| Crate | Version | Declared license | Used under |')
[void]$out.AppendLine('|---|---|---|---|')
foreach ($r in $rows) { [void]$out.AppendLine($r) }
[void]$out.AppendLine()
[void]$out.AppendLine('## License texts')
foreach ($k in $entries.Keys) {
    [void]$out.AppendLine()
    [void]$out.AppendLine("### $($entries[$k] -join ', ')")
    [void]$out.AppendLine()
    [void]$out.AppendLine('```text')
    [void]$out.AppendLine((($k -replace '@@MIT@@', $mitText.Trim()) -replace '@@BSL@@', $bslText.Trim()))
    [void]$out.AppendLine('```')
}
if ($apache) {
    [void]$out.AppendLine()
    [void]$out.AppendLine('## Apache License, Version 2.0')
    [void]$out.AppendLine()
    [void]$out.AppendLine('```text')
    [void]$out.AppendLine($apache)
    [void]$out.AppendLine('```')
}
[System.IO.File]::WriteAllText((Join-Path (Get-Location) 'THIRD_PARTY_LICENSES.md'), $out.ToString(), (New-Object System.Text.UTF8Encoding $false))
"Wrote THIRD_PARTY_LICENSES.md: $($crates.Count) crates, $($entries.Count) distinct license texts."
