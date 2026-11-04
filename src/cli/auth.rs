use crate::api::NanitApiClient;
use anyhow::{bail, Context, Result};
use inquire::{Password, Text};

pub async fn run_auth(email_opt: Option<String>, password_opt: Option<String>) -> Result<()> {
    println!("\n🔐 --- Nanit 2FA Authentication Setup ---\n");

    let email = match email_opt {
        Some(e) if !e.trim().is_empty() => e,
        _ => Text::new("Nanit account email:")
            .prompt()
            .context("Failed to read email")?,
    };

    let password = match password_opt {
        Some(p) if !p.trim().is_empty() => p,
        _ => Password::new("Nanit account password:")
            .without_confirmation()
            .prompt()
            .context("Failed to read password")?,
    };

    let api = NanitApiClient::new()?;
    let login_res = api.initiate_login(&email, &password).await?;

    let (access_token, refresh_token) = if let Some(mfa_token) = login_res.mfa_token {
        let suffix = login_res.phone_suffix.unwrap_or_else(|| "**".to_string());
        let channel = login_res.channel.unwrap_or_else(|| "sms".to_string());

        println!("\n📱 MFA Challenge Detected!");
        println!("A verification code was sent via {channel} to your phone ending in ...{suffix}.");

        let code = Text::new("Enter 4-digit verification code:")
            .prompt()
            .context("Failed to read MFA code")?;

        let mfa_res = api
            .complete_mfa(&email, &password, &mfa_token, code.trim(), &channel)
            .await?;

        let refresh = mfa_res
            .refresh_token
            .context("Server did not return a refresh token after MFA")?;
        let access = mfa_res.access_token.unwrap_or_default();

        (access, refresh)
    } else if let Some(refresh) = login_res.refresh_token {
        let access = login_res.access_token.unwrap_or_default();
        (access, refresh)
    } else {
        bail!("Unexpected response from Nanit API: No MFA challenge and no tokens returned.");
    };

    println!("\n🎉 Successfully authenticated with Nanit!");
    println!("--------------------------------------------------");
    println!("NANIT_REFRESH_TOKEN={refresh_token}");
    println!("--------------------------------------------------");
    println!("\nStore this token in your .env file or 1Password vault.");
    println!("The daemon will use this token to generate ephemeral access sessions automatically without triggering SMS codes.");

    // Bonus: Check registered babies
    if !access_token.is_empty() {
        if let Ok(babies) = api.get_babies(&access_token).await {
            println!("\n👶 Discovered Cameras / Babies:");
            for baby in babies {
                let name = baby.name.as_deref().unwrap_or("Baby");
                let cam = baby.camera_uid.as_deref().unwrap_or("unknown");
                println!(
                    "  • Name: {name} | Baby UID: {} | Camera UID: {cam}",
                    baby.uid
                );
            }
        }
    }

    Ok(())
}
