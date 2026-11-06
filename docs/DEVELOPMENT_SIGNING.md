# Development Code Signing Guide (`DEV_TEST_SIGNED`)

## 1. Overview and Purpose

Cyron Uninstaller provides a dedicated, reproducible local code-signing workflow for Windows development, regression testing, and controlled virtual machine (VM) evaluation:

- **Classification:** `DEV_TEST_SIGNED`
- **Signer Subject:** `CN=Cyron Uninstaller Development`
- **Certificate Store:** `Cert:\CurrentUser\My`
- **Key Specification:** RSA 3072-bit, SHA-256, CodeSigning EKU (`1.3.6.1.5.5.7.3.3`)
- **Tooling:** Official Windows 10/11 SDK `signtool.exe`

This workflow allows developers to verify that PE executables (`cyron.exe`), NSIS installer packages (`.exe`), and WiX Windows Installer packages (`.msi`) can be properly signed, that signatures survive installer bundling, and that cryptographic file integrity checks function correctly before public release.

---

## 2. Hard Security Restrictions

> [!WARNING]
> Self-signed development certificates are **strictly for local development and controlled testing environments**. Follow these rules without exception:

1. **No User Root Store Contamination:** Development certificates must **never** be automatically imported into the `Trusted Root Certification Authorities` or `Trusted Publishers` store on a user's machine.
2. **No Policy Circumvention:** Never disable, bypass, or alter Windows Defender, Microsoft Defender SmartScreen, or Windows 11 Smart App Control policies to simulate signing success.
3. **No False Public Claims:** Self-signed development builds must **never** be labelled as publicly trusted or distributed as general production releases.
4. **No Private Key Distribution:** No private key, PFX bundle, or certificate password may ever be committed to git, published to GitHub Actions, or packaged into release archives. Keys are generated non-exportable in `Cert:\CurrentUser\My`.
5. **SmartScreen Reality:** Self-signed certificates **cannot** eliminate Windows SmartScreen or Smart App Control warnings on third-party computers. SmartScreen reputation requires public trust chains and telemetry from Microsoft's cloud services.

---

## 3. Workflow Commands

All scripts integrate directly with the project root and MSVC environment:

```powershell
# 1. Create or inspect the local development certificate (idempotent; reuses existing valid cert)
powershell -ExecutionPolicy Bypass -File scripts/create-dev-cert.ps1

# 2. Build unsigned release bundles
npm run build:unsigned

# 3. Build and automatically sign release bundles with the dev certificate
npm run build:dev-signed

# 4. Verify signatures of all built artifacts (includes automated negative test suite)
npm run verify:signatures

# 5. Verify Windows PE metadata, MSI database properties, and NSIS installer headers
npm run verify:metadata
```

---

## 4. How Verification Works

Authenticode verification in `scripts/verify-signatures.ps1` separates **cryptographic integrity** from **trust chain authority**:

1. **Cryptographic Integrity (Digest Validation):**
   - The SHA-256 digest embedded in the binary's Authenticode directory is verified against the binary's actual hash.
   - If any byte of the signed binary is tampered with, `Get-AuthenticodeSignature` and `signtool.exe verify` report `HashMismatch` or `NotSigned`.
2. **Signer Identity Validation:**
   - The certificate thumbprint and subject are extracted and compared against `CN=Cyron Uninstaller Development`. Unrelated or rogue certificates are rejected.
3. **Trust Chain Evaluation:**
   - In a standard developer environment, the signature status will be `UnknownError` with the message:
     `A certificate chain processed, but terminated in a root certificate which is not trusted by the trust provider.`
   - This status is **expected and normal** for self-signed certificates, confirming that the signature is structurally and cryptographically sound while respecting Windows trust boundaries.

### Automated Negative Tests

Running `npm run verify:signatures` executes three automated negative test scenarios:
- **Negative Test 1:** Unsigned binaries are rejected as unsigned.
- **Negative Test 2:** Tampered signed binaries are detected and rejected.
- **Negative Test 3:** Binaries signed with certificates bearing incorrect subjects (e.g., `CN=Wrong Subject Attacker`) are rejected.

---

## 5. Controlled Testing in an Isolated VM

When performing end-to-end installer tests (e.g., testing elevated installer prompts or Windows SmartScreen dialog behaviors) on an isolated disposable Windows test VM:

1. **Export the Public Certificate (Public Key Only, No Private Key):**
   ```powershell
   $cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -eq "CN=Cyron Uninstaller Development" } | Select-Object -First 1
   Export-Certificate -Cert $cert -FilePath "$env:TEMP\cyron-dev-public.cer"
   ```
2. **Import Public Certificate into the Disposable VM's Root Store:**
   ```powershell
   # RUN ONLY INSIDE AN ISOLATED DISPOSABLE TEST VM:
   Import-Certificate -FilePath "cyron-dev-public.cer" -CertStoreLocation "Cert:\LocalMachine\Root"
   ```
3. **Run Install and Verify:**
   In the test VM, `Get-AuthenticodeSignature` will report `Status: Valid`, and the installer will display `Cyron Uninstaller Development` as a verified publisher during UAC prompts.
4. **Dispose or Reset the VM:** Revert the VM snapshot after testing. Never perform this on physical host workstations.
