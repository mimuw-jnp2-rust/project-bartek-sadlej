# Chat App

## Autorzy
- Barte Sadlej 

## Opis
Komunikator napisany w ruscie

## Funkcjonalność
1)
- [x] Serwer tworzy podane jako argumenty kanały 
- [x] Użytkownicy przy dołączaniu podaje nik i nazwe kanału do którego chcą dołączyć
- [x] Wiadomości są widoczne w obrębie jednego kanału
- [x] Można albo wysłać wiadomośc albo zmienić kanał

2)
- Dodanie logowania i tworzenia użytkowników 
- Wczytywanie i zapisywanie kanałów i użytkowników
- Tworzenie nowych kanałów i pamiętanie kanałów urzytkowników 
- Dodanie historii oraz wysyłanie nieprzeczytanych wiadomości, wysłanych od ostatniego logowania 

- Wszystko raczej z terminala
## Propozycja podziału na części
W pierwszej części pierwsze cztery punkty, 

W drugiej części kolejne cztery punkty, 

## Biblioteki
- Tokio : wątki obsługujące i połączenie i server tcp
- Serde : do serializacji komunikatów (to chyba dobry pomysł?)
