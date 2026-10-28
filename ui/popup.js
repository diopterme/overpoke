const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// "오늘은 그만"은 실수로 누르면 자정까지 꺼지므로 두 번 눌러야 적용
const MUTE_CONFIRM_MS = 3000;
const MUTE_LABEL = '오늘은 그만';
const MUTE_ARMED_LABEL = '한 번 더 누르면 오늘 끔';

const timeEl = document.getElementById('time');
const textEl = document.getElementById('text');
const muteBtn = document.getElementById('mute');
let muteTimer = null;

const showError = e => { textEl.textContent = `오류: ${e}`; };
const call = cmd => invoke(cmd).catch(showError);

function disarmMute() {
  clearTimeout(muteTimer);
  muteTimer = null;
  muteBtn.textContent = MUTE_LABEL;
  muteBtn.classList.remove('armed');
}

async function refresh() {
  disarmMute();
  try {
    // "19:00 초과근무 기록 버튼을 누르세요" → 시각과 나머지를 나눠 표시
    const [time, ...rest] = (await invoke('popup_message')).split(' ');
    timeEl.textContent = time;
    textEl.textContent = rest.join(' ');
  } catch (e) {
    showError(e);
  }
}

muteBtn.addEventListener('click', () => {
  if (muteTimer === null) {
    muteBtn.textContent = MUTE_ARMED_LABEL;
    muteBtn.classList.add('armed');
    muteTimer = setTimeout(disarmMute, MUTE_CONFIRM_MS);
    return;
  }
  disarmMute();
  call('mute_today');
});

document.getElementById('ok').addEventListener('click', () => call('close_popup'));

document.addEventListener('keydown', e => {
  if (e.key === 'Enter' || e.key === 'Escape') {
    // 포커스된 버튼의 기본 동작(예: 무장된 "오늘은 그만")보다 우선
    e.preventDefault();
    call('close_popup');
  }
});

listen('popup-refresh', refresh);
refresh();
