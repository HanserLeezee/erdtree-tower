param(
    [Parameter(Mandatory=$true)][string]$Entry,
    [ValidateSet('Solo','Seamless')][string]$Mode = 'Seamless',
    [ValidateSet('ME3','YAFSML')][string]$Loader = 'ME3',
    [ValidateSet('Steam','OnlineFix')][string]$Network = 'Steam',
    [Parameter(Mandatory=$true)][string]$GameDir,
    [Parameter(Mandatory=$true)][string]$ErrorFile,
    [Parameter(Mandatory=$true)][string]$LogDirectory
)
$ErrorActionPreference = 'Stop'
try {
    [void][IO.Directory]::CreateDirectory($LogDirectory)
    foreach ($module in @('Microsoft.PowerShell.Management','Microsoft.PowerShell.Utility')) {
        Import-Module ([IO.Path]::Combine($PSHOME,'Modules',$module,($module + '.psd1'))) -ErrorAction Stop
    }
    function Quote-Argument([string]$Value) {
        '"' + [regex]::Replace([regex]::Replace($Value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1') + '"'
    }
    $arguments = @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',(Quote-Argument $Entry),
        '-Mode',$Mode,'-GameDir',(Quote-Argument $GameDir),'-ErrorFile',(Quote-Argument $ErrorFile),'-Loader',$Loader,'-Network',$Network)
    $shell = (Get-Process -Id $PID).Path
    $worker = Start-Process -FilePath $shell -ArgumentList $arguments -WorkingDirectory (Split-Path -Parent $Entry) -WindowStyle Hidden -RedirectStandardOutput (Join-Path $LogDirectory 'stdout.log') -RedirectStandardError (Join-Path $LogDirectory 'stderr.log') -PassThru
    $handle = $worker.Handle
    $worker.WaitForExit()
    $code = $worker.ExitCode
    [IO.File]::WriteAllText((Join-Path $LogDirectory 'exit-code.txt'),[string]$code)
    exit $code
} catch {
    [IO.File]::WriteAllText($ErrorFile,$_.Exception.Message)
    exit 1
}
