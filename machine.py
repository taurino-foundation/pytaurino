"""Original measurement process, independent of the GUI."""

from __future__ import annotations

import random
from enum import Enum
from typing import Any

import numpy as np


class imp_data:
    def __init__(self):
        self.file_name = "name"
        self.probe_nr = "0"
        self.comments = "no comment"
        self.u_min = 0.0
        self.u_max = 3.0
        self.delta_u = 0.2
        self.t_meas = 20000.0  # ms
        self.I = 0.0
        self.U = 0.0
        self.modus = 0
        self.SMachine = 0
        self.error = 0
        self.sig_sn = "0000"
        self.firmware = "V1.0"
        self.time_p = 0.0
        self.progress = 0.0

    def server_data(self):
        data = np.array(
            [
                self.SMachine,
                self.modus,
                self.error,
                self.progress,
                self.file_name,
                self.probe_nr,
                self.comments,
                self.u_min,
                self.u_max,
                self.delta_u,
                self.t_meas,
                self.I,
                self.U,
                self.time_p,
                self.sig_sn,
                self.firmware,
            ],
            dtype=object,
        )
        return data


class SM(Enum):
    INIT = 0
    READY = 1
    RUNNING = 2
    MEASURING = 3
    PAUSE = 4
    STOP = 5
    CANCEL = 6


class S_DATA(Enum):
    STATE_MACHINE = 0
    MODUS = 1
    ERROR = 2
    PROGRESS = 3
    FILE_NAME = 4
    PROBE_NR = 5
    COMMENTS = 6
    U_MIN = 7
    U_MAX = 8
    DELTA_U = 9
    T_MEAS = 10
    I = 11  # noqa: E741
    U = 12
    TIME_P = 13
    SIG_SN = 14
    FIRMWARE = 15


class classMultiPros:
    def __init__(self):
        self.init = 0

    def th(self, server_data: Any, stop_event: Any) -> None:
        while not stop_event.is_set():
            SMach = int(server_data[S_DATA.STATE_MACHINE.value])

            if SMach == SM.INIT.value:
                print("vai")

                if stop_event.wait(10):
                    return

                server_data[S_DATA.STATE_MACHINE.value] = SM.READY.value

            elif SMach == SM.RUNNING.value:
                if stop_event.wait(2):
                    return

                server_data[S_DATA.STATE_MACHINE.value] = SM.MEASURING.value

            elif SMach == SM.MEASURING.value:
                if stop_event.wait(10):
                    return

                server_data[S_DATA.STATE_MACHINE.value] = SM.STOP.value

            elif SMach == SM.CANCEL.value:
                if stop_event.wait(10):
                    return

                server_data[S_DATA.STATE_MACHINE.value] = SM.READY.value

            elif SMach == SM.STOP.value:
                if stop_event.wait(10):
                    return

                server_data[S_DATA.STATE_MACHINE.value] = SM.READY.value

            server_data[S_DATA.I.value] = random.randint(0, 10)
            server_data[S_DATA.U.value] = random.randint(0, 10)

            if stop_event.wait(1):
                return
