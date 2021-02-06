const EventEmitter = require('events');
const childProcess = require('child_process');
const jspb = require('google-protobuf');
const protos = require('../generated/blockfish_pb.js');

class IPC extends EventEmitter {
    constructor(subprocess) {
        super();
        Object.assign(this, {
            subprocess,
            _len: null,
            _killed: false,
        });
        subprocess.stdout.on('readable', this._onReadable.bind(this));
        subprocess.on('exit', this._onProcessExit.bind(this));
        subprocess.on('error', e => this.emit('error', e));
    }

    _onReadable() {
        let bufs = [];
        let buf;
        while ((buf = this.subprocess.stdout.read()) !== null) {
            bufs.push(buf);
        }
        buf = Buffer.concat(bufs);
        let pos = this._read(buf);
        if (pos < buf.length) {
            this.subprocess.stdout.unshift(buf.slice(pos));
        }
    }

    _read(buf) {
        if (this._killed) {
            // ignore all read buffers
            return buf.length;
        }
        let pos = 0;
        for (;;) {
            if (this._len === null) {
                if (!anyVarints(buf, pos)) {
                    break;
                }
                let reader = new jspb.BinaryReader(buf, pos);
                this._len = reader.decoder_.readUnsignedVarint32();
                pos = reader.getCursor();
            } else {
                if (buf.length - pos < this._len) {
                    break;
                }
                let reader = new jspb.BinaryReader(buf, pos, this._len);
                let resp = new protos.Response;
                protos.Response.deserializeBinaryFromReader(resp, reader);
                this.emit('recv', resp);
                this._len = null;
                pos = reader.getCursor();
            }
        }
        return pos;
    }

    _onProcessExit(sig) {
        if (typeof sig === 'number' && sig !== 0) {
            this.emit('error', new Error("non-zero exit code " + sig));
        } else {
            this.emit('exit');
        }
    }

    send(req, cb) {
        if (this._killed) {
            return;
        }
        const chunk = req.serializeBinary();
        const lengthWriter = new jspb.BinaryWriter;
        lengthWriter.encoder_.writeUnsignedVarint32(chunk.length);
        const lengthChunk = lengthWriter.getResultBuffer();
        this.subprocess.stdin.write(lengthChunk);
        this.subprocess.stdin.write(chunk, cb);
    }

    kill() {
        if (this._killed) {
            return;
        }
        this.subprocess.kill('SIGTERM');
        this._killed = true;
    }
}

function anyVarints(buf, pos) {
    for (let i = pos; i < buf.length; i++) {
        if (!(buf[i] >>> 7)) {
            return true;
        }
    }
    return false;
}

IPC.childProcess = (executable, args) => new IPC(
    childProcess.spawn(
        executable,
        args,
        { stdio: ['pipe', 'pipe', 'inherit'] },
    )
);

module.exports = IPC;
