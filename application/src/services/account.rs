use kernel::entities::{Account, Address, NonVerifiedAccount, Password, TicketId, UpdatedAt, UserId, UserName, VerificationCode};
use kernel::external::{OffsetDateTime, Uuid};
use kernel::KernelError;
use kernel::repository::{
    DependOnAccountRepository,
    DependOnNonVerifiedAccountRepository,
    AccountRepository,
    NonVerifiedAccountRepository
};

#[allow(unused_imports)]
use kernel::transporter::{
    DependOnVerificationMailTransporter,
    VerificationMailTransporter
};

use crate::{
    ApplicationError,
    transfer::account::{
        CreateNonVerifiedAccountDto,
        NonVerifiedAccountDto,
        CreateAccountDto,
        UpdateAccountDto,
        AccountDto,
    }
};

#[async_trait::async_trait]
pub trait CreateNonVerifiedAccountService: 'static + Send + Sync
    + DependOnNonVerifiedAccountRepository
    + DependOnVerificationMailTransporter
{
    async fn create(&self, create: CreateNonVerifiedAccountDto) -> Result<NonVerifiedAccountDto, ApplicationError> {
        let ticket = TicketId::default();
        let code = VerificationCode::default();
        let CreateNonVerifiedAccountDto { address } = create;
        let non_verified = NonVerifiedAccount::new(ticket, address, code);

        self.non_verified_account_repository().create(&non_verified).await?;

        #[cfg(not(debug_assertions))]
        self.verification_mail_transporter().send(non_verified.code(), non_verified.address()).await?;

        #[cfg(debug_assertions)]
        println!("{:?}", non_verified.code());

        Ok(non_verified.into())
    }
}

impl<T> CreateNonVerifiedAccountService for T
    where T: DependOnNonVerifiedAccountRepository
           + DependOnVerificationMailTransporter {}

pub trait DependOnCreateNonVerifiedAccountService: 'static + Send + Sync {
    type CreateNonVerifiedAccountService: CreateNonVerifiedAccountService;
    fn create_non_verified_account_service(&self) -> &Self::CreateNonVerifiedAccountService;
}

#[async_trait::async_trait]
pub trait ApproveAccountService: 'static + Send + Sync
    + DependOnNonVerifiedAccountRepository
{
    async fn approve(&self, id: &str, code: &str) -> Result<String, ApplicationError> {
        let id = TicketId::new(id);
        let code = VerificationCode::new(code);

        let Some(nonverified) = self.non_verified_account_repository().find_by_id(&id).await? else {
            return Err(ApplicationError::NotFound {
                method: "find",
                entity: "non-verified account",
                id: id.into()
            })
        };

        if !nonverified.is_match_verification_code(&code) {
            return Err(ApplicationError::InvalidValue {
                method: "2FA code verify",
                value: "verification code".into()
            });
        };

        let valid = TicketId::default();

        self.non_verified_account_repository().validation(&id, &valid, nonverified.address()).await?;

        Ok(valid.into())
    }
}

impl<T> ApproveAccountService for T
    where T: DependOnNonVerifiedAccountRepository {}

pub trait DependOnApproveAccountService: 'static + Send + Sync {
    type ApproveAccountService: ApproveAccountService;
    fn approve_account_service(&self) -> &Self::ApproveAccountService;
}

#[async_trait::async_trait]
pub trait CreateAccountService: 'static + Send + Sync
    + DependOnAccountRepository
    + DependOnNonVerifiedAccountRepository
{
    async fn create(&self, id: &str, create: CreateAccountDto) -> Result<AccountDto, ApplicationError> {
        let id = TicketId::new(id);

        let Some(verified) = self.non_verified_account_repository()
            .find_by_valid_id(&id).await? else {
            return Err(ApplicationError::NotFound {
                method: "find",
                entity: "temporary account",
                id: id.into()
            })
        };

        let CreateAccountDto { name, pass } = create;

        let verified = Account::new(
            UserId::default(),
            verified,
            name,
            pass,
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc(),
            OffsetDateTime::now_utc()
        )?;

        self.account_repository().create(&verified).await?;

        Ok(verified.into())
    }
}

impl<T> CreateAccountService for T
    where T: DependOnAccountRepository
           + DependOnNonVerifiedAccountRepository {}

pub trait DependOnCreateAccountService: 'static + Send + Sync {
    type CreateAccountService: CreateAccountService;
    fn create_account_service(&self) -> &Self::CreateAccountService;
}

#[async_trait::async_trait]
pub trait UpdateAccountService: 'static + Send + Sync
    + DependOnAccountRepository
{
    async fn update(&self, update: UpdateAccountDto) -> Result<AccountDto, ApplicationError> {
        let UpdateAccountDto { id, address, name, pass } = update;
        let id = UserId::new(id);
        let Some(account) = self.account_repository()
            .find_by_id(&id).await? else {
            return Err(ApplicationError::NotFound {
                method: "update",
                entity: "Account",
                id: id.as_ref().to_string()
            })
        };

        account.pass().verify(&pass)
            .map_err(|e| match e {
                KernelError::Cryption(e) => ApplicationError::External(anyhow::Error::new(e)),
                KernelError::InvalidPassword(_) => ApplicationError::Verification {
                    method: "on update in verification",
                    entity: "account",
                    id: id.as_ref().to_string()
                },
                _ => unreachable!()
            })?;

        let mut account = account.into_destruct();
        let mut date = account.date.into_destruct();

        account.address = Address::new(address);
        account.name = UserName::new(name);
        account.pass = Password::new(pass)?;

        date.updated_at = UpdatedAt::new(OffsetDateTime::now_utc());
        let date = date.freeze();

        account.date = date;

        let account = account.freeze();

        self.account_repository().update(&account).await?;

        Ok(account.into())
    }
}

impl<T> UpdateAccountService for T
    where T: DependOnAccountRepository {}

pub trait DependOnUpdateAccountService: 'static + Send + Sync {
    type UpdateAccountService: UpdateAccountService;
    fn update_account_service(&self) -> &Self::UpdateAccountService;
}

#[async_trait::async_trait]
pub trait DeleteAccountService: 'static + Send + Sync
    + DependOnAccountRepository
{
    async fn delete(&self, pass: &str, delete: &Uuid) -> Result<(), ApplicationError> {
        let id = UserId::new(*delete);
        let Some(account) = self.account_repository()
            .find_by_id(&id).await? else {
            return Err(ApplicationError::NotFound {
                method: "delete",
                entity: "account",
                id: id.as_ref().to_string()
            })
        };

        account.pass().verify(pass)
            .map_err(|e| match e {
                KernelError::Cryption(e) => ApplicationError::External(anyhow::Error::new(e)),
                KernelError::InvalidPassword(_) => ApplicationError::Verification {
                    method: "on delete in verification",
                    entity: "account",
                    id: id.as_ref().to_string()
                },
                _ => unreachable!()
            })?;

        self.account_repository().delete(&id).await?;

        Ok(())
    }
}

impl<T> DeleteAccountService for T
    where T: DependOnAccountRepository {}

pub trait DependOnDeleteAccountService: 'static + Send + Sync {
    type DeleteAccountService: DeleteAccountService;
    fn delete_account_repository(&self) -> &Self::DeleteAccountService;
}