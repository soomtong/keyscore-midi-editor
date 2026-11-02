// CI 시험 단계 동안 CoreMIDI 클라이언트 하나를 쥐고 있어 MIDI 서버가 내려가지 않게 한다.
//
// MIDIServices.h: 마지막 클라이언트가 사라지면 MIDI 서버가 끝날 수 있다. 시험 바이너리는 저마다
// 프로세스라, 바이너리가 끝날 때마다 서버가 내려가고 다음 바이너리의 첫 MIDIClientCreate가 서버
// 재기동을 기다린다. 원격 CI 첫 실행에서 이 기다림이 10초 재시도(CLIENT_CREATE_PATIENCE)를 넘겨
// -50으로 실패했다. 사람의 Mac에서는 보통 다른 MIDI 앱이 클라이언트를 쥐고 있어 드러나지 않는다.
import CoreMIDI
import Foundation

var client = MIDIClientRef()
let status = MIDIClientCreate("keyscore-ci-keepalive" as CFString, nil, nil, &client)
guard status == noErr else {
    FileHandle.standardError.write("MIDIClientCreate \(status)\n".data(using: .utf8)!)
    exit(1)
}
print("ready")
fflush(stdout)
// RunLoop.main.run()은 run loop에 입력 소스가 없으면 곧바로 돌아온다(리뷰 1 지적 1). 조건 없이 막힌다.
while true {
    sleep(3600)
}
