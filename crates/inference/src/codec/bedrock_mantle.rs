//! Amazon Bedrock Mantle's OpenAI Responses transport contract.
//!
//! Mantle deliberately reuses the canonical Responses mapping while retaining
//! its distinct AWS endpoint and credential-rejection behavior.

use omp_core::Str;
use url::Url;

use super::{
	Codec, DecodeContext, Decoder, DecoderState, EncodeContext, EncodedRequest, Frame, RawEvent,
	openai_responses::{OpenAiResponsesCodec, OpenAiResponsesOptions},
};
use crate::{
	auth::AuthScheme,
	call::OperationCall,
	error::{Error, RetryAction},
};

const REGION_PLACEHOLDER: &str = "{region}";

/// OpenAI Responses mapping with Bedrock Mantle authentication recovery.
#[derive(Clone, Debug, Default)]
pub struct BedrockMantleCodec {
	inner: OpenAiResponsesCodec,
}

impl BedrockMantleCodec {
	/// Constructs a Mantle codec with the canonical Responses options.
	pub const fn new(options: OpenAiResponsesOptions) -> Self {
		Self { inner: OpenAiResponsesCodec::new(options) }
	}
}

impl Codec for BedrockMantleCodec {
	fn encode(
		&self,
		context: &EncodeContext<'_>,
		operation: &OperationCall,
	) -> Result<EncodedRequest, Error> {
		self.inner.encode(context, operation)
	}

	fn decoder(&self, context: &DecodeContext<'_>) -> Result<DecoderState, Error> {
		Ok(Box::new(BedrockMantleDecoder {
			inner:         self.inner.decoder(context)?,
			refresh_sigv4: context.auth_scheme == Some(AuthScheme::AwsSigV4),
		}))
	}
}

struct BedrockMantleDecoder {
	inner:         DecoderState,
	refresh_sigv4: bool,
}

fn map_auth_rejection(refresh_sigv4: bool, mut error: Error) -> Error {
	const AUTH_CODES: &[&str] = &[
		"401",
		"403",
		"invalid_api_key",
		"authentication_error",
		"unauthorized",
		"permission_denied",
		"authorization_error",
	];
	let rejected = matches!(error.status, Some(401 | 403))
		|| error.code.as_deref().is_some_and(|code| {
			AUTH_CODES
				.iter()
				.any(|expected| code.eq_ignore_ascii_case(expected))
		});
	if refresh_sigv4 && !error.committed && rejected {
		error.action = RetryAction::RefreshCredentialOnce;
	}
	error
}

impl Decoder for BedrockMantleDecoder {
	fn push(&mut self, frame: Frame, emit: &mut dyn FnMut(RawEvent)) -> Result<(), Error> {
		let refresh_sigv4 = self.refresh_sigv4;
		let mut mapped_emit = |event| match event {
			RawEvent::Failure(error) => {
				emit(RawEvent::Failure(map_auth_rejection(refresh_sigv4, error)));
			},
			other => emit(other),
		};
		self
			.inner
			.push(frame, &mut mapped_emit)
			.map_err(|error| map_auth_rejection(refresh_sigv4, error))
	}

	fn finish(&mut self, emit: &mut dyn FnMut(RawEvent)) -> Result<(), Error> {
		let refresh_sigv4 = self.refresh_sigv4;
		let mut mapped_emit = |event| match event {
			RawEvent::Failure(error) => {
				emit(RawEvent::Failure(map_auth_rejection(refresh_sigv4, error)));
			},
			other => emit(other),
		};
		self
			.inner
			.finish(&mut mapped_emit)
			.map_err(|error| map_auth_rejection(refresh_sigv4, error))
	}

	fn is_complete(&self) -> bool {
		self.inner.is_complete()
	}

	fn prepare_browser_retry(&mut self) -> bool {
		self.inner.prepare_browser_retry()
	}

	fn supports_control(&self) -> bool {
		self.inner.supports_control()
	}

	fn encode_control(
		&mut self,
		input: super::ProviderControlInput,
	) -> Result<Option<bytes::Bytes>, Error> {
		self.inner.encode_control(input)
	}
}

/// Expands one catalog-owned Mantle endpoint with a validated AWS region.
pub fn expand_endpoint(base: &str, region: &str) -> Result<Str, BedrockMantleEndpointError> {
	if region.is_empty()
		|| !region
			.bytes()
			.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
	{
		return Err(BedrockMantleEndpointError::InvalidRegion);
	}
	let expanded = base.replace(REGION_PLACEHOLDER, region);
	let parsed = Url::parse(&expanded).map_err(BedrockMantleEndpointError::Url)?;
	if parsed.scheme() != "https" {
		return Err(BedrockMantleEndpointError::InsecureEndpoint);
	}
	if parsed.host_str().is_none() {
		return Err(BedrockMantleEndpointError::MissingHost);
	}
	Ok(Str::new(parsed.to_string()))
}

/// Structural failure while expanding a catalog Mantle endpoint.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BedrockMantleEndpointError {
	/// The region is empty or contains characters not valid in an AWS region.
	#[error("Bedrock Mantle region is invalid")]
	InvalidRegion,
	/// The expanded endpoint is not a valid URL.
	#[error("Bedrock Mantle endpoint URL is invalid")]
	Url(#[source] url::ParseError),
	/// Mantle endpoints must use TLS.
	#[error("Bedrock Mantle endpoint must use HTTPS")]
	InsecureEndpoint,
	/// The expanded endpoint has no host.
	#[error("Bedrock Mantle endpoint has no host")]
	MissingHost,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		error::{ErrorKind, ErrorPhase},
		receipt::ExecutionReceipt,
	};

	#[test]
	fn endpoint_expands_region_without_changing_the_responses_base_path() {
		let endpoint =
			expand_endpoint("https://bedrock-mantle.{region}.api.aws/openai/v1", "eu-west-2")
				.expect("valid regional endpoint");
		assert_eq!(endpoint.as_str(), "https://bedrock-mantle.eu-west-2.api.aws/openai/v1",);
	}

	#[test]
	fn endpoint_rejects_host_injection_through_region() {
		assert_eq!(
			expand_endpoint(
				"https://bedrock-mantle.{region}.api.aws/openai/v1",
				"us-east-1.evil.example",
			),
			Err(BedrockMantleEndpointError::InvalidRegion),
		);
	}

	#[test]
	fn sigv4_authentication_rejection_refreshes_once_before_output() {
		let mut error = Error::new(
			ErrorKind::Authorization,
			ErrorPhase::Handshake,
			RetryAction::Never,
			ExecutionReceipt::default(),
		);
		error.code = Some(Str::new_static("permission_denied"));
		assert_eq!(map_auth_rejection(true, error).action, RetryAction::RefreshCredentialOnce,);
	}
}
