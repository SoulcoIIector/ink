use crate::{
	state::{
		ContractState,
		EmptyContractState,
	},
	exec_env::{
		ExecutionEnv,
	},
	msg::{
		Message,
	},
	msg_handler::{
		UnreachableMessageHandler,
		MessageHandler,
		MessageHandlerMut,
		RawMessageHandler,
		RawMessageHandlerMut,
	},
};
use core::marker::PhantomData;

/// A marker struct to tell that the deploy handler requires no arguments.
#[derive(Copy, Clone)]
pub struct NoDeployArgs;

/// A handler specific to deploying a smart contract.
///
/// # Note
///
/// This is normally mainly used to correctly initialize
/// a smart contracts state.
pub struct DeployHandler<State, Args> {
	/// The arguments that deploy expects.
	///
	/// This tricks Rust into thinking that this owns the state type.
	/// However, it is just a marker which allows the contract declaration
	/// to be a zero-sized-type (ZST).
	args: PhantomData<Args>,
	/// The actual deployment function.
	deploy_fn: fn(&mut ExecutionEnv<State>, Args),
}

impl<State> DeployHandler<State, NoDeployArgs> {
	/// Returns a deploy handler that does nothing.
	const fn init() -> Self {
		Self {
			args: PhantomData,
			deploy_fn: move |env, _| {},
		}
	}
}

impl<State, Args> DeployHandler<State, Args> {
	/// Returns a new deploy handler for the given closure.
	const fn new(raw_handler: fn(&mut ExecutionEnv<State>, Args)) -> Self {
		Self {
			args: PhantomData,
			deploy_fn: raw_handler,
		}
	}
}

impl<State, Args> Copy for DeployHandler<State, Args> {}

impl<State, Args> Clone for DeployHandler<State, Args> {
	fn clone(&self) -> Self {
		Self {
			args: self.args,
			deploy_fn: self.deploy_fn,
		}
	}
}

/// A contract declaration.
///
/// Uses the builder pattern in order to represent a contract
/// based on compile-time construction.
///
/// Can be used to actually instantiate a contract during run-time
/// in order to dispatch a contract call or deploy state.
pub struct ContractDecl<
	State,
	DeployArgs,
	HandlerChain,
> {
	/// The type of the contract state.
	///
	/// This tricks Rust into thinking that this owns the state type.
	/// However, it is just a marker which allows the contract declaration
	/// to be a zero-sized-type (ZST).
	state: PhantomData<State>,

	deployer: DeployHandler<State, DeployArgs>,
	/// The compile-time chain of message handlers.
	///
	/// # Note
	///
	/// They are represented by a recursive tuple chain and start with
	/// a simple `UnreachableMessageHandler` node. For every further
	/// registered message handler this tuple is extended recursively.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// UnreachableMessageHandler               // Upon initialization.
	/// (Foo, UnreachableMessageHandler)        // After adding message handler Foo.
	/// (Bar, (Foo, UnreachableMessageHandler)) // After adding message handler Bar.
	/// ```
	///
	/// Note that every pair of message handlers is also a message handler.
	handlers: HandlerChain,
}

impl<State, DeployArgs, HandlerChain> Clone for ContractDecl<State, DeployArgs, HandlerChain>
where
	HandlerChain: Clone,
{
	fn clone(&self) -> Self {
		Self {
			state: self.state,
			deployer: self.deployer,
			handlers: self.handlers.clone(),
		}
	}
}

impl<State, DeployArgs, HandlerChain> Copy for ContractDecl<State, DeployArgs, HandlerChain>
where
	HandlerChain: Copy,
{}

impl ContractDecl<EmptyContractState, NoDeployArgs, UnreachableMessageHandler> {
	/// Creates a new contract declaration with the given name.
	pub fn new<State>() -> ContractDecl<State, NoDeployArgs, UnreachableMessageHandler> {
		ContractDecl {
			state: PhantomData,
			deployer: DeployHandler::init(),
			handlers: UnreachableMessageHandler,
		}
	}
}

impl<State> ContractDecl<State, NoDeployArgs, UnreachableMessageHandler> {
	/// Registers the given deployment procedure for the contract.
	///
	/// # Note
	///
	/// This is used to initialize the contract state upon deployment.
	pub const fn on_deploy<Args>(
		self,
		handler: fn(&mut ExecutionEnv<State>, Args),
	)
		-> ContractDecl<State, Args, UnreachableMessageHandler>
	where
		Args: parity_codec::Decode,
	{
		ContractDecl {
			state: self.state,
			deployer: DeployHandler::new(handler),
			handlers: self.handlers,
		}
	}
}

impl<State, DeployArgs, HandlerChain> ContractDecl<State, DeployArgs, HandlerChain>
where
	Self: Copy, // Required in order to make this compile-time computable.
{
	/// Convenience method to append another message handler.
	const fn append_msg_handler<MsgHandler>(self, handler: MsgHandler)
		-> ContractDecl<State, DeployArgs, (MsgHandler, HandlerChain)>
	{
		ContractDecl {
			state: PhantomData,
			deployer: self.deployer,
			handlers: (handler, self.handlers)
		}
	}

	/// Registers a read-only message handler.
	///
	/// # Note
	///
	/// Read-only message handlers do not mutate contract state.
	pub const fn on_msg<Msg>(
		self,
		handler: RawMessageHandler<Msg, State>,
	)
		-> ContractDecl<State, DeployArgs, (MessageHandler<Msg, State>, HandlerChain)>
	where
		Msg: Message,
		State: ContractState,
	{
		self.append_msg_handler(MessageHandler::from_raw(handler))
	}

	/// Registers a mutable message handler.
	///
	/// # Note
	///
	/// Mutable message handlers may mutate contract state.
	pub const fn on_msg_mut<Msg>(
		self,
		handler: RawMessageHandlerMut<Msg, State>,
	)
		-> ContractDecl<State, DeployArgs, (MessageHandlerMut<Msg, State>, HandlerChain)>
	where
		Msg: Message,
		State: ContractState,
	{
		self.append_msg_handler(MessageHandlerMut::from_raw(handler))
	}
}