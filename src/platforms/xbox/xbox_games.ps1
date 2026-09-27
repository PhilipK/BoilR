Get-AppxPackage |
Where-Object { -not $_.IsFramework } |
ForEach-Object {
    try {
        $manifest = Get-AppxPackageManifest $_
        $application = $manifest.Package.Applications.Application;
        [PSCustomObject]@{
            kind             = $application.Id
            display_name     = $manifest.Package.Properties.DisplayName
            install_location = $_.InstallLocation
            family_name      = $_.PackageFamilyName
        }
    }
    catch {}
} |
ConvertTo-Json -depth 5