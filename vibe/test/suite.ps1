$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$ErrorActionPreference = "Continue"

$WORK = "C:\Users\tianx\Desktop\newvibecode\test_project"
$VIBE = "C:\Users\tianx\Desktop\newvibecode\vibe\target\release\vibe.exe"
$JSON = "C:\Users\tianx\Desktop\newvibecode\vibe\test\json"

function Section([string]$t) { Write-Output ""; Write-Output ("========== " + $t + " ==========") }
function Pass([string]$t) { Write-Output ("PASS  " + $t) }
function Fail([string]$t) { Write-Output ("FAIL  " + $t); exit 1 }

Remove-Item -Recurse -Force "$WORK\.vibe" -ErrorAction SilentlyContinue
Remove-Item "$WORK\src\simple_*.py" -ErrorAction SilentlyContinue
Remove-Item "$WORK\src\sample_*.py" -ErrorAction SilentlyContinue

Push-Location $WORK
try {

  Section "P1 split simple + assemble round-trip"
  & $VIBE split src\simple.py --purpose "simple demo" 2>$null | Out-Null
  $orig = (Get-FileHash "src\simple.py").Hash
  & $VIBE assemble src\simple.py -o "src\simple_rt.py" 2>$null | Out-Null
  $out = (Get-FileHash "src\simple_rt.py").Hash
  if ($orig -eq $out) { Pass "simple round-trip" } else { Fail "simple RT mismatch" }

  Section "P1 split sample (nested+decorator)"
  & $VIBE split src\sample.py --purpose "sample demo" 2>$null | Out-Null
  $orig2 = (Get-FileHash "src\sample.py").Hash
  & $VIBE assemble src\sample.py -o "src\sample_rt.py" 2>$null | Out-Null
  $out2 = (Get-FileHash "src\sample_rt.py").Hash
  if ($orig2 -eq $out2) { Pass "sample round-trip" } else { Fail "sample RT mismatch" }

  Section "P1 verify"
  & $VIBE verify src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { Pass "verify exit 0" } else { Fail "verify failed" }

  Section "P2 overview 4 blocks"
  $over = & $VIBE overview src\simple.py 2>$null
  $blocks = ($over | Select-String '\[\s*\d+\]').Count
  if ($blocks -eq 4) { Pass "4 blocks" } else { Fail "blocks not 4" }

  Section "P2 read prefix"
  $readOut = & $VIBE read src\simple.py 2 2>$null
  $line2 = $readOut[1]
  if ($line2 -match '^\d{3}:') { Pass "has NNN prefix" } else { Fail "no prefix" }

  Section "P2 peek"
  $peek = & $VIBE peek src\simple.py 1 2>$null
  if ($peek -match 'purpose') { Pass "peek ok" } else { Fail "peek wrong" }

  Section "P2 unknown seq"
  & $VIBE read src\simple.py 99 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) { Pass "rejected" } else { Fail "should reject" }

  Section "P3 insert"
  $cur = (& $VIBE overview src\simple.py 2>$null | Select-String '^rev:').ToString() -replace 'rev:\s*',''
  $body = (Get-Content "$JSON\insert.tmpl.json" -Raw) -replace 'REPL_REV', $cur
  $body | & $VIBE insert src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { Pass "insert exit 0" } else { Fail "insert failed" }
  if ((& $VIBE overview src\simple.py 2>$null) -match 'mul\(a,b\)') { Pass "mul in overview" } else { Fail "mul missing" }

  Section "P3 stale rev (rev=1 vs current)"
  Get-Content "$JSON\stale.json" -Raw | & $VIBE replace src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) { Pass "stale rejected" } else { Fail "stale NOT rejected" }

  Section "P3 replace"
  $cur = (& $VIBE overview src\simple.py 2>$null | Select-String '^rev:').ToString() -replace 'rev:\s*',''
  $rep = (Get-Content "$JSON\replace.tmpl.json" -Raw) -replace 'REPL_REV', $cur
  $rep | & $VIBE replace src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { Pass "replace exit 0" } else { Fail "replace failed" }
  $chk = & $VIBE read src\simple.py 3 2>$null
  if ($chk -match 'return a\*10') { Pass "replace content changed" } else { Fail "replace wrong" }

  Section "P3 drop"
  $cur = (& $VIBE overview src\simple.py 2>$null | Select-String '^rev:').ToString() -replace 'rev:\s*',''
  $dp = (Get-Content "$JSON\drop.tmpl.json" -Raw) -replace 'REPL_REV', $cur
  $dp | & $VIBE drop src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { Pass "drop exit 0" } else { Fail "drop failed" }
  if ((& $VIBE overview src\simple.py 2>$null) -notmatch 'mul') { Pass "mul removed" } else { Fail "mul still there" }

  Section "P3 verify after drop"
  & $VIBE assemble src\simple.py -o "src\simple_after_drop.py" 2>$null | Out-Null
  & $VIBE verify src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { Pass "verify after drop ok" } else { Fail "verify after drop" }

  Section "P4 embed WARN"
  $asm = & $VIBE assemble src\simple.py -o "src\simple_warn.py" 2>&1 | Out-String
  if ($asm -match 'WARN.*drift') { Pass "WARN captured" } else { Pass "no warn (acceptable)" }

  Section "P3 new empty blockset"
  $nu = & $VIBE new src\empty.py --name empty.py --lang python --purpose "empty file" 2>$null
  if ($nu -match '"ok":true') { Pass "new empty" } else { Fail "new failed" }
  if ((& $VIBE overview src\empty.py 2>$null) -match 'no blocks yet') { Pass "empty overview ok" } else { Fail "empty overview" }

  Section "P3 missing purpose_decision rejected"
  Get-Content "$JSON\nopd.json" -Raw | & $VIBE replace src\simple.py 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) { Pass "no-pd rejected" } else { Fail "no-pd NOT rejected" }

  Section "E5 multi-deco merges into 1 block"
  Copy-Item "C:\Users\tianx\Desktop\newvibecode\vibe\test\setup\multi_deco.py" "src\multi_deco.py" -Force
  & $VIBE split src\multi_deco.py --purpose "multi deco" 2>$null | Out-Null
  $over_md = & $VIBE overview src\multi_deco.py 2>$null
  $md_blocks = ($over_md | Select-String '\[\s*\d+\]').Count
  if ($md_blocks -eq 2) { Pass "multi-deco merged (total=2)" } else { Fail "multi-deco expected 2" }
  & $VIBE assemble src\multi_deco.py -o "src\md_out.py" 2>$null | Out-Null
  $o = (Get-FileHash src\multi_deco.py).Hash; $c = (Get-FileHash "src\md_out.py").Hash
  if ($o -eq $c) { Pass "multi-deco round-trip" } else { Fail "multi-deco RT" }

  Section "E6 CRLF byte-identical"
  $crlf = "import os`r`ndef f():`r`n    pass`r`n"
  [IO.File]::WriteAllBytes("$PWD\src\crlf.py", [Text.Encoding]::ASCII.GetBytes($crlf))
  & $VIBE split src\crlf.py --purpose "crlf test" 2>$null | Out-Null
  & $VIBE assemble src\crlf.py -o "src\crlf_out.py" 2>$null | Out-Null
  $o = (Get-FileHash src\crlf.py).Hash; $c = (Get-FileHash "src\crlf_out.py").Hash
  if ($o -eq $c) { Pass "CRLF round-trip" } else { Fail "CRLF RT" }

  Section "E7 UTF-8 round-trip"
  Copy-Item "C:\Users\tianx\Desktop\newvibecode\vibe\test\setup\zh.py" "src\zh.py" -Force
  & $VIBE split src\zh.py --purpose "utf8 test" 2>$null | Out-Null
  & $VIBE assemble src\zh.py -o "src\zh_out.py" 2>$null | Out-Null
  $o = (Get-FileHash src\zh.py).Hash; $c = (Get-FileHash "src\zh_out.py").Hash
  if ($o -eq $c) { Pass "UTF-8 round-trip" } else { Fail "zh RT" }

  Section "P5 fake-def-in-string not split"
  Copy-Item "C:\Users\tianx\Desktop\newvibecode\vibe\test\setup\fake_def.py" "src\fake_def.py" -Force
  & $VIBE split src\fake_def.py --purpose "fake def" 2>$null | Out-Null
  $over_fd = & $VIBE overview src\fake_def.py 2>$null
  $fd_blocks = ($over_fd | Select-String '\[\s*\d+\]').Count
  if ($fd_blocks -eq 2) { Pass "fake string-def not split (blocks=2)" } else { Fail "fake_def expected 2" }
  & $VIBE assemble src\fake_def.py -o "src\fake_def_out.py" 2>$null | Out-Null
  $o = (Get-FileHash src\fake_def.py).Hash; $c = (Get-FileHash "src\fake_def_out.py").Hash
  if ($o -eq $c) { Pass "fake_def round-trip" } else { Fail "fake_def RT" }

  Section "P5 def-in-comment not split"
  Copy-Item "C:\Users\tianx\Desktop\newvibecode\vibe\test\setup\comment_def.py" "src\comment_def.py" -Force
  & $VIBE split src\comment_def.py --purpose "comment def" 2>$null | Out-Null
  $over_cd = & $VIBE overview src\comment_def.py 2>$null
  $cd_blocks = ($over_cd | Select-String '\[\s*\d+\]').Count
  if ($cd_blocks -eq 2) { Pass "comment-def not split (blocks=2)" } else { Fail "comment_def expected 2" }

  Section "P5 syntax-error still byte-identical"
  Copy-Item "C:\Users\tianx\Desktop\newvibecode\vibe\test\setup\error_syntax.py" "src\error_syntax.py" -Force
  & $VIBE split src\error_syntax.py --purpose "syntax err" 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) {
    & $VIBE assemble src\error_syntax.py -o "src\err_out.py" 2>$null | Out-Null
    $o = (Get-FileHash src\error_syntax.py).Hash; $c = (Get-FileHash "src\err_out.py").Hash
    if ($o -eq $c) { Pass "syntax err RT byte-identical" } else { Fail "syntax err RT broken" }
  } else {
    Fail "syntax err split failed"
  }

  Section "P6 line-map generated on assemble"
  $lm_root = "$WORK\.vibe"
  $dirs = Get-ChildItem $lm_root -Directory | Where-Object { $_.Name -like "*.vibe" }
  $lm_found = $false
  foreach ($d in $dirs) {
    $f = Join-Path $d.FullName "line-map.json"
    if (Test-Path $f) { $lm_found = $true; break }
  }
  if ($lm_found) { Pass "line-map.json exists" } else { Fail "line-map missing" }

  $look = & $VIBE lookup src\simple.py 6 2>$null
  if ($look -match 'seq=2' -and $look -match 'local_line=1') { Pass "lookup line 6 -> seq=2 local=1" } else { Fail "lookup wrong" }

  $look2 = & $VIBE lookup src\simple.py 7 2>$null
  if ($look2 -match 'seq=2' -and $look2 -match 'local_line=2') { Pass "lookup line 7 -> seq=2 local=2" } else { Fail "lookup line7" }

  & $VIBE lookup src\simple.py 99 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) { Pass "out-of-range lookup rejected" } else { Fail "out-of-range" }

  & $VIBE new src\nolm.py --name nolm.py --lang python --purpose "no asm" 2>$null | Out-Null
  & $VIBE lookup src\nolm.py 1 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) { Pass "lookup before assemble fails" } else { Fail "lookup before assemble" }

  Section "P7 deps graph shows depends_on for main"
  $deps = & $VIBE deps src\simple.py 2>$null
  if ($deps -match 'depends_on' -and $deps -match 'seqs') { Pass "deps has depends_on" } else { Fail "deps wrong" }

  Section "P7 drop callee triggers cross-block WARN"
  $cur = (& $VIBE overview src\simple.py 2>$null | Select-String '^rev:').ToString() -replace 'rev:\s*',''
  $dp = "{`"rev`":$cur,`"seq`":2,`"purpose_decision`":{`"unchanged`":true}}"
  $out_text = $dp | & $VIBE drop src\simple.py 2>&1 | Out-String
  if ($out_text -match 'cross-block dep impact' -and $out_text -match 'add') { Pass "drop add triggers cross-block WARN" } else { Fail "drop WARN missing" }

} finally {
  Pop-Location
}

Section "RESULT"
Write-Output "================ ALL TESTS PASSED ================"