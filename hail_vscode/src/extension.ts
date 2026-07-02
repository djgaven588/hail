import * as path from 'path';
import * as vscode from 'vscode';
import { ExtensionContext } from 'vscode';
import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
	const config = vscode.workspace.getConfiguration('hail');
	const customServerPath = config.get<string>('serverPath') || '';

	let serverBin: string;
	if (customServerPath) {
		serverBin = path.isAbsolute(customServerPath)
			? customServerPath
			: context.asAbsolutePath(customServerPath);
	} else {
		// TODO: THIS PATH NEEDS TO BE BETTER
		serverBin = context.asAbsolutePath(path.join('..', 'target', 'release', 'hail-lsp'));
	}

	const serverOptions: ServerOptions = {
		command: serverBin,
		args: ['--lsp'],
		transport: TransportKind.stdio,
		options: { env: Object.assign({}, process.env, { RUST_BACKTRACE: '1' }) }
	};

	const debugOptionsExtended = Object.assign({}, serverOptions, {
		options: Object.assign({}, (serverOptions as any).options, { execArgv: ['--nolazy', '--inspect=6009'] })
	});

	const clientOptions: LanguageClientOptions = {
		documentSelector: [{ language: 'hail' }],
		diagnosticCollectionName: 'hail',
		initializationOptions: {}
	};

	client = new LanguageClient(
		'hail-language-server',
		'Hail Language Server',
		serverOptions,
		clientOptions
	);

	client.start();
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
