import { mkdir, readFile, appendFile } from 'node:fs/promises';
import { dirname } from 'node:path';
export const appendHookEvent = async (filePath, event) => {
    await mkdir(dirname(filePath), { recursive: true });
    await appendFile(filePath, `${JSON.stringify(event)}\n`, 'utf8');
};
export const readHookEvents = async (filePath) => {
    try {
        const raw = await readFile(filePath, 'utf8');
        return raw
            .split('\n')
            .filter((line) => line.trim().length > 0)
            .map((line) => JSON.parse(line));
    }
    catch (error) {
        if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
            return [];
        }
        throw error;
    }
};
