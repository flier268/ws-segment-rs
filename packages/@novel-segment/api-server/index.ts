import { spawn, ChildProcess } from 'child_process';
import { existsSync } from 'fs';
import { resolve } from 'path';

function rustBin(): string {
	const candidates = [
		resolve(__dirname, '../../../target/release/novel-segment'),
		resolve(__dirname, '../../../target/debug/novel-segment'),
	];
	for (const p of candidates)
	{
		if (existsSync(p))
		{
			return p;
		}
	}
	return 'novel-segment';
}

export function listen(bind?: string): ChildProcess
{
	return spawn(rustBin(), ['serve', '--bind', bind || '127.0.0.1:3000'], {
		stdio: 'inherit',
	});
}

export default { listen };
