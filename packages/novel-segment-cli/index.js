"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SegmentCliError = exports.stringify = exports.enableDebug = void 0;
exports.textSegmentCore = textSegmentCore;
exports.textSegment = textSegment;
exports.fileSegmentCore = fileSegmentCore;
exports.fileSegment = fileSegment;
exports.processText = processText;
exports.processTextCore = processTextCore;
exports.processFile = processFile;
exports.readFile = readFile;
exports.fixOptions = fixOptions;
exports.getCacache = getCacache;
exports.resetSegment = resetSegment;
exports.getSegment = getSegment;
exports.loadCacheInfo = loadCacheInfo;
exports.loadCacheDb = loadCacheDb;
exports.removeCache = removeCache;
exports.resetCache = resetCache;
const tslib_1 = require("tslib");
/**
 * novel-segment-cli - 命令列分詞工具
 * Novel Segment CLI - Command-line segmentation tool
 *
 * 提供文字與檔案的分詞處理功能，支援快取管理、簡繁轉換。
 * Provides text and file segmentation processing with cache management and Traditional/Simplified Chinese conversion.
 */
const crlf_normalize_1 = tslib_1.__importDefault(require("crlf-normalize"));
const novel_segment_1 = tslib_1.__importDefault(require("novel-segment"));
const bluebird_1 = tslib_1.__importDefault(require("bluebird"));
const fs_iconv_1 = require("fs-iconv");
const cache_1 = tslib_1.__importDefault(require("./lib/cache"));
const util_1 = require("./lib/util");
Object.defineProperty(exports, "enableDebug", { enumerable: true, get: function () { return util_1.enableDebug; } });
const package_json_1 = tslib_1.__importDefault(require("./package.json"));
const util_2 = require("novel-segment/lib/util");
const iconv_jschardet_1 = tslib_1.__importDefault(require("iconv-jschardet"));
const min_1 = require("@lazy-cjk/zh-convert/min");
const lodash_1 = require("lodash");
const array_hyper_unique_1 = require("array-hyper-unique");
/**
 * 快取的分詞實例
 * Cached Segment Instance
 *
 * 使用單例模式快取分詞器實例，避免重複初始化。
 * Uses singleton pattern to cache the segmenter instance to avoid repeated initialization.
 *
 * @private
 */
let CACHED_SEGMENT;
/**
 * 快取的 Cacache 實例
 * Cached Cacache Instance
 *
 * 用於儲存字典資料的快取系統。
 * Used to store dictionary data cache.
 *
 * @private
 */
let CACHED_CACACHE;
/**
 * 一般模式的快取資料庫鍵名
 * Cache database key for normal mode
 *
 * @constant {string}
 */
const DB_KEY = 'cache.db';
/**
 * 一般模式的快取資訊鍵名
 * Cache info key for normal mode
 *
 * @constant {string}
 */
const DB_KEY_INFO = 'cache.info';
/**
 * 小說模式（nodeNovelMode）的快取資料庫鍵名
 * Cache database key for novel mode (nodeNovelMode)
 *
 * @constant {string}
 */
const DB_KEY2 = 'cache.common.synonym.db';
/**
 * 小說模式的快取資訊鍵名
 * Cache info key for novel mode
 *
 * @constant {string}
 */
const DB_KEY2_INFO = 'cache.common.synonym.info';
/**
 * 快取預設 TTL（Time To Live）
 * Default cache TTL (Time To Live)
 *
 * 預設為 1 小時（3600 秒 = 3600 * 1000 毫秒）。
 * Default is 1 hour (3600 seconds = 3600 * 1000 milliseconds).
 *
 * @constant {number}
 */
const DB_TTL = 3600 * 1000;
/**
 * 分詞結果序列化函數
 * Segmentation result serialization function
 *
 * 將分詞後的資料結構轉換為文字。
 * Converts segmented data structure to text.
 *
 * @constant
 */
const stringify = novel_segment_1.default.stringify;
exports.stringify = stringify;
/**
 * 核心文字分詞函數
 * Core text segmentation function
 *
 * 執行實際的分詞處理，將文字分割為詞語陣列。
 * Performs actual segmentation processing, splitting text into word arrays.
 *
 * @param segment - 分詞器實例 / Segmenter instance
 * @param text - 待分詞的文字 / Text to segment
 * @param options - 選項物件 / Options object
 * @returns 分詞結果 / Segmentation result
 */
function textSegmentCore(segment, text, options) {
    return bluebird_1.default.resolve(segment)
        .then(function (segment) {
        return segment.doSegment(text);
    })
        .tap(function (data) {
        return (0, util_2.debug_token)(data);
    });
}
/**
 * 文字分詞函數
 * Text segmentation function
 *
 * 取得分詞器並對文字進行分詞處理的便捷函數。
 * Convenience function to get segmenter and perform text segmentation.
 *
 * @param text - 待分詞的文字 / Text to segment
 * @param options - 選項物件 / Options object
 * @returns 分詞結果 / Segmentation result
 */
function textSegment(text, options) {
    return textSegmentCore(getSegment(options), text, options);
}
/**
 * 核心檔案分詞函數
 * Core file segmentation function
 *
 * 讀取檔案並對其內容進行分詞處理。
 * Reads a file and performs segmentation on its content.
 *
 * @param segment - 分詞器實例 / Segmenter instance
 * @param file - 檔案路徑 / File path
 * @param options - 選項物件 / Options object
 * @returns 分詞結果 / Segmentation result
 */
function fileSegmentCore(segment, file, options) {
    return bluebird_1.default.resolve(readFile(file))
        .then(function (buf) {
        return textSegmentCore(segment, buf.toString(), options);
    });
}
/**
 * 檔案分詞函數
 * File segmentation function
 *
 * 取得分詞器並對檔案內容進行分詞處理的便捷函數。
 * Convenience function to get segmenter and perform file content segmentation.
 *
 * @param file - 檔案路徑 / File path
 * @param options - 選項物件 / Options object
 * @returns 分詞結果 / Segmentation result
 */
function fileSegment(file, options) {
    return getSegment(options)
        .then((segment) => {
        return fileSegmentCore(segment, file, options);
    });
}
/**
 * 處理文字並返回格式化結果
 * Process text and return formatted result
 *
 * 對文字進行分詞、格式化（換行符號、簡繁轉換）並返回最終結果。
 * Performs segmentation, formatting (line breaks, Traditional/Simplified conversion) on text and returns final result.
 *
 * @param text - 待處理的文字 / Text to process
 * @param options - 選項物件 / Options object
 * @returns 處理後的文字 / Processed text
 */
function processText(text, options) {
    return processTextCore(getSegment(options), text, options);
}
/**
 * 核心處理文字函數
 * Core process text function
 *
 * 執行文字處理的核心邏輯，包括分詞、換行符號處理、簡繁轉換。
 * Executes core text processing logic including segmentation, line break handling, Traditional/Simplified conversion.
 *
 * @param segment - 分詞器實例 / Segmenter instance
 * @param text - 待處理的文字 / Text to process
 * @param options - 選項物件 / Options object
 * @returns 處理後的文字 / Processed text
 */
function processTextCore(segment, text, options) {
    // 檢查文字是否為空或只包含空白字元 / Check if text is empty or contains only whitespace
    if (!text.length || !text.replace(/\s+/g, '').length) {
        return bluebird_1.default.resolve('');
    }
    return textSegmentCore(segment, text, options)
        .then(function (data) {
        let text = stringify(data);
        if (options) {
            // 處理換行符號格式 / Handle line break character formatting
            if (options.crlf) {
                if (typeof options.crlf === 'string') {
                    text = (0, crlf_normalize_1.default)(text, options.crlf);
                }
                else {
                    text = (0, crlf_normalize_1.default)(text);
                }
            }
            // 轉換為繁體中文 / Convert to Traditional Chinese
            if (options.convertToZhTw) {
                text = (0, min_1.cn2tw_min)(text);
            }
        }
        // 釋放記憶體 / Free memory
        (0, util_1.freeGC)();
        return text;
    });
}
/**
 * 處理檔案並返回格式化結果
 * Process file and return formatted result
 *
 * 讀取檔案內容，進行分詞處理並返回格式化後的結果。
 * Reads file content, performs segmentation processing and returns formatted result.
 *
 * @param file - 檔案路徑 / File path
 * @param options - 選項物件 / Options object
 * @returns 處理後的文字 / Processed text
 */
function processFile(file, options) {
    return bluebird_1.default.resolve(readFile(file, options))
        .then(function (buf) {
        return processText(buf.toString(), options);
    });
}
/**
 * CLI 分詞錯誤
 * CLI Segmentation Error
 *
 * 用於表示 novel-segment-cli 操作中發生的錯誤。
 * Used to represent errors occurring during novel-segment-cli operations.
 */
class SegmentCliError extends Error {
}
exports.SegmentCliError = SegmentCliError;
/**
 * 讀取檔案
 * Read file
 *
 * 讀取指定檔案並自動偵測編碼，回傳 Buffer 物件。
 * Reads specified file and auto-detects encoding, returns Buffer object.
 *
 * @param file - 檔案路徑 / File path
 * @param options - 選項物件 / Options object
 * @returns 檔案內容的 Buffer / Buffer of file content
 */
function readFile(file, options) {
    return bluebird_1.default.resolve().then(() => {
        // 檢查檔案是否存在 / Check if file exists
        if (!(0, fs_iconv_1.existsSync)(file)) {
            let e = new SegmentCliError(`ENOENT: no such file or directory, open '${file}'`);
            return bluebird_1.default.reject(e);
        }
        // 載入檔案並自動解碼 / Load file and auto-decode
        return (0, fs_iconv_1.loadFile)(file, {
            autoDecode: true,
        })
            .then(v => Buffer.from(v));
    })
        .tap(function (buf) {
        // 若已停用警告則跳過 / Skip if warnings are disabled
        if (options && options.disableWarn) {
            return;
        }
        // 檢查檔案是否為空 / Check if file is empty
        if (!buf.length) {
            util_1.console.warn(`此檔案無內容`, file);
        }
        else {
            // 偵測檔案編碼 / Detect file encoding
            let chk = iconv_jschardet_1.default.detect(buf);
            // 警告非 UTF-8 編碼的檔案 / Warn about non-UTF-8 encoded files
            if (chk.encoding != 'UTF-8' && chk.encoding != 'ascii') {
                util_1.console.warn('此檔案可能不是 UTF8 請檢查編碼或利用 MadEdit 等工具轉換', chk, file);
            }
        }
    });
}
/**
 * 修正與補全選項
 * Fix and complete options
 *
 * 處理選項物件，設定預設值並根據 mode 選擇正確的快取鍵名。
 * Processes options object, sets default values and selects correct cache key based on mode.
 *
 * @param options - 原始選項物件 / Original options object
 * @returns 修正後的選項物件 / Fixed options object
 */
function fixOptions(options) {
    options = options || {};
    // 驗證並修正 TTL 值 / Validate and fix TTL value
    if (typeof options.ttl !== 'number' || options.ttl < 1) {
        delete options.ttl;
    }
    // 確保分詞選項物件存在 / Ensure segment options object exists
    options.optionsSegment = options.optionsSegment || {};
    // 根據 mode 選擇對應的快取鍵名 / Select corresponding cache key based on mode
    if (options.optionsSegment.nodeNovelMode) {
        options.USER_DB_KEY = options.USER_DB_KEY || DB_KEY2;
        options.USER_DB_KEY_INFO = options.USER_DB_KEY_INFO || DB_KEY2_INFO;
    }
    else {
        options.USER_DB_KEY = options.USER_DB_KEY || DB_KEY;
        options.USER_DB_KEY_INFO = options.USER_DB_KEY_INFO || DB_KEY_INFO;
    }
    return options;
}
/**
 * 取得 Cacache 實例
 * Get Cacache instance
 *
 * 回傳快取管理實例，若不存在則建立新的單例。
 * Returns cache management instance, creates new singleton if not exists.
 *
 * @param options - 選項物件 / Options object
 * @returns Cacache 實例 / Cacache instance
 */
function getCacache(options) {
    return new bluebird_1.default(function (resolve, reject) {
        if (!CACHED_CACACHE) {
            // 使用全域快取或本地位址快取 / Use global cache or local temp cache
            if (options && options.useGlobalCache) {
                CACHED_CACACHE = new cache_1.default({
                    name: package_json_1.default.name,
                    useGlobalCache: options.useGlobalCache,
                    autoCreateDir: true,
                });
            }
            else {
                CACHED_CACACHE = new cache_1.default({
                    name: package_json_1.default.name,
                    autoCreateDir: true,
                });
            }
        }
        resolve(CACHED_CACACHE);
    });
}
/**
 * 重置分詞器
 * Reset segmenter
 *
 * 清除快取的分詞器實例，強制下次重新初始化。
 * Clears cached segmenter instance, forces re-initialization on next use.
 */
function resetSegment() {
    CACHED_SEGMENT = void 0;
}
/**
 * 取得分詞器實例
 * Get segmenter instance
 *
 * 回傳已初始化並快取的分詞器實例。包含完整的字典載入與快取邏輯。
 * Returns initialized and cached segmenter instance. Includes complete dictionary loading and caching logic.
 *
 * 處理流程：
 * 1. 載入或建立 Cacache 實例
 * 2. 檢查是否有可用的快取字典
 * 3. 比對版本號，若版本不同則重新初始化
 * 4. 載入黑名單與同義詞字典
 * 5. 建立最終的字典資料表
 * 6. 儲存更新後的快取（若需要）
 *
 * Processing flow:
 * 1. Load or create Cacache instance
 * 2. Check if cached dictionary is available
 * 3. Compare version numbers, re-initialize if version differs
 * 4. Load blacklist and synonym dictionaries
 * 5. Build final dictionary table
 * 6. Save updated cache (if needed)
 *
 * @param options - 選項物件 / Options object
 * @returns 分詞器實例 / Segmenter instance
 */
function getSegment(options) {
    options = fixOptions(options);
    return bluebird_1.default
        .resolve()
        .then(async function () {
        const optionsSegment = {
            autoCjk: true,
            optionsDoSegment: {
                convertSynonym: true,
            },
            all_mod: true,
            ...options.optionsSegment,
        };
        if (!CACHED_SEGMENT) {
            const { create } = require('../novel-segment-native');
            CACHED_SEGMENT = create({
                autoCjk: optionsSegment.autoCjk !== false,
                allMod: optionsSegment.all_mod !== false,
                nodeNovelMode: !!(optionsSegment.nodeNovelMode),
                convertSynonym: optionsSegment.optionsDoSegment?.convertSynonym !== false,
                optionsDoSegment: optionsSegment.optionsDoSegment,
            });
            util_1.debugConsole.debug(`使用 Rust native 分詞核心`);
        }
        return CACHED_SEGMENT;
    });
}
/**
 * 載入快取資訊
 * Load cache info
 *
 * 讀取快取的中繼資訊（版本、大小等），用於判斷快取有效性。
 * Reads cached metadata (version, size, etc.), used to determine cache validity.
 *
 * @param options - 選項物件 / Options object
 * @returns 快取資訊物件 / Cache info object
 */
function loadCacheInfo(options) {
    return bluebird_1.default
        .resolve()
        .then(async function () {
        await getCacache(options);
        // 檢查快取資訊是否存在 / Check if cache info exists
        let has_cache_db = await CACHED_CACACHE.hasData(options.USER_DB_KEY_INFO);
        let data;
        if (has_cache_db) {
            // 讀取快取資訊 / Read cache info
            data = await CACHED_CACACHE
                .readJSON(options.USER_DB_KEY_INFO)
                .then(function (ret) {
                return ret.json;
            });
        }
        // 初始化預設值 / Initialize default values
        data = data || {};
        data.last = data.last || {};
        data.current = data.current || {};
        data.last.version = data.last.version || {};
        data.current.version = data.current.version || {};
        // 清除原型鏈以避免效能問題 / Clear prototype chain to avoid performance issues
        if (data.DICT) {
            Object.setPrototypeOf(data.DICT, null);
        }
        return data;
    });
}
/**
 * 載入快取資料庫
 * Load cache database
 *
 * 讀取儲存的字典資料庫，包含完整的字典內容。
 * Reads stored dictionary database, containing complete dictionary content.
 *
 * @param options - 選項物件 / Options object
 * @returns 快取資料庫或 null / Cache database or null
 */
function loadCacheDb(options) {
    options = fixOptions(options);
    let { disableCache } = options;
    // 若停用快取則回傳 null / Return null if cache is disabled
    if (disableCache) {
        return bluebird_1.default
            .resolve(null);
    }
    return bluebird_1.default
        .resolve()
        .then(async function () {
        await getCacache(options);
        // 檢查快取是否存在且尚未過期 / Check if cache exists and not expired
        let has_cache_db = await CACHED_CACACHE.hasData(options.USER_DB_KEY, {
            ttl: options.ttl > 0 ? options.ttl : DB_TTL,
        });
        if (has_cache_db) {
            util_1.debugConsole.debug(`發現緩存 ${options.USER_DB_KEY}`, has_cache_db.path);
            return CACHED_CACACHE
                .readJSON(options.USER_DB_KEY)
                .then(function (ret) {
                var _a;
                // 清除原型鏈 / Clear prototype chain
                if ((_a = ret.json) === null || _a === void 0 ? void 0 : _a.DICT) {
                    Object.setPrototypeOf(ret.json.DICT, null);
                }
                return ret.json;
            });
        }
        return null;
    });
}
/**
 * 移除快取
 * Remove cache
 *
 * 清除所有相關的快取資料，包括不同模式（一般/小說）的快取。
 * Clears all related cache data, including different modes (normal/novel) caches.
 *
 * @param options - 選項物件 / Options object
 */
function removeCache(options) {
    let opts = fixOptions(options);
    return bluebird_1.default.all((0, array_hyper_unique_1.array_unique)([
        opts,
        (0, lodash_1.merge)({}, opts, {
            optionsSegment: {
                nodeNovelMode: true,
            },
        }),
        (0, lodash_1.merge)({}, opts, {
            optionsSegment: {
                nodeNovelMode: false,
            },
        }),
    ]))
        .map(async (o) => {
        const cache = await getCacache(o);
        await cache.clearMemoized();
        await cache.removeAll();
    });
}
/**
 * 重置快取
 * Reset cache
 *
 * 清除快取的 Cacache 實例，強制下次重新建立。
 * Clears cached Cacache instance, forces re-creation on next use.
 */
function resetCache() {
    CACHED_CACACHE = void 0;
}
//# sourceMappingURL=index.js.map