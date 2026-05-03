[CmdletBinding()]
param(
  [string]$BaseBranch = "yuholy",
  [string]$UpstreamBranch = "upstream/main",
  [int]$MaxCommits = 20
)

$ErrorActionPreference = "Stop"

Write-Host "Fetching remotes..." -ForegroundColor Cyan
git fetch origin --prune
git fetch upstream --prune

Write-Host ""
Write-Host "Base branch: $BaseBranch" -ForegroundColor Yellow
Write-Host "Upstream branch: $UpstreamBranch" -ForegroundColor Yellow

Write-Host ""
Write-Host "Ahead/behind summary:" -ForegroundColor Cyan
git rev-list --left-right --count "$BaseBranch...$UpstreamBranch"

Write-Host ""
Write-Host "Upstream commits not in ${BaseBranch}:" -ForegroundColor Cyan
git log --oneline "--max-count=$MaxCommits" "$BaseBranch..$UpstreamBranch"

Write-Host ""
Write-Host "Diff stat:" -ForegroundColor Cyan
git diff --stat "$BaseBranch..$UpstreamBranch"

Write-Host ""
Write-Host "Review complete. Selectively port only the changes we want." -ForegroundColor Green
